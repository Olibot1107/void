use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::ast::{BinaryOp, Expr, Stmt, UnaryOp};
use crate::lexer;
use crate::parser::Parser;
use crate::value::{new_object, Env, EnvRef, Function, Value};

pub struct Runtime {
    module_cache: HashMap<PathBuf, Value>,
    stdlib: HashMap<String, Value>,
    argv: Vec<String>,
}

enum ExecFlow {
    Continue,
    Return(Value),
}

#[derive(Debug, Deserialize)]
struct ModuleManifest {
    main: Option<String>,
    module: Option<String>,
    entry: Option<String>,
}

static VOID_ID_COUNTER: AtomicU64 = AtomicU64::new(1);
static VOID_START: OnceLock<Instant> = OnceLock::new();

impl Runtime {
    pub fn new(argv: Vec<String>) -> Self {
        let _ = VOID_START.get_or_init(Instant::now);
        let mut runtime = Self {
            module_cache: HashMap::new(),
            stdlib: HashMap::new(),
            argv,
        };
        runtime.stdlib = runtime.build_stdlib();
        runtime
    }

    pub fn run_entry(&mut self, entry: &Path) -> Result<(), String> {
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        let entry_path = resolve_path(&cwd, entry);
        self.load_module(&entry_path).map(|_| ())
    }

    fn load_module(&mut self, file_path: &Path) -> Result<Value, String> {
        let target_path = resolve_existing_module(file_path)
            .ok_or_else(|| format!("Module not found: {}", file_path.display()))?;
        let resolved = fs::canonicalize(&target_path)
            .map_err(|e| format!("{} ({})", e, target_path.display()))?;

        if let Some(cached) = self.module_cache.get(&resolved) {
            return Ok(cached.clone());
        }

        let source = fs::read_to_string(&resolved)
            .map_err(|e| format!("Failed to read {}: {e}", resolved.display()))?;

        let tokens = lexer::lex(&source)?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program()?;

        let env = Env::new(None);
        Env::define(&env, "say", native_wrap(native_say));
        Env::define(&env, "print", native_wrap(native_say));
        let console_obj = new_object();
        set_object_prop(&console_obj, "log", native_wrap(native_say)).expect("console.log");
        set_object_prop(&console_obj, "error", native_wrap(native_console_error))
            .expect("console.error");
        Env::define(&env, "console", console_obj);
        Env::define(&env, "__filename", Value::from_str(&resolved.display().to_string()));
        if let Some(parent) = resolved.parent() {
            Env::define(&env, "__dirname", Value::from_str(&parent.display().to_string()));
        }

        let exports_obj = new_object();
        let module_obj = new_object();
        set_object_prop(&module_obj, "exports", exports_obj.clone())?;
        Env::define(&env, "module", module_obj);
        Env::define(&env, "exports", exports_obj.clone());

        let module_dir = resolved
            .parent()
            .ok_or_else(|| format!("No parent directory for {}", resolved.display()))?;

        for stmt in &program {
            match self.execute_stmt(stmt, &env, module_dir)? {
                ExecFlow::Continue => {}
                ExecFlow::Return(_) => {
                    return Err("Return is not allowed at module top-level".to_string())
                }
            }
        }

        self.module_cache.insert(resolved, exports_obj.clone());
        Ok(exports_obj)
    }

    fn execute_block(
        &mut self,
        block: &[Stmt],
        env: &EnvRef,
        module_dir: &Path,
    ) -> Result<ExecFlow, String> {
        for stmt in block {
            match self.execute_stmt(stmt, env, module_dir)? {
                ExecFlow::Continue => {}
                ExecFlow::Return(value) => return Ok(ExecFlow::Return(value)),
            }
        }
        Ok(ExecFlow::Continue)
    }

    fn execute_stmt(
        &mut self,
        stmt: &Stmt,
        env: &EnvRef,
        module_dir: &Path,
    ) -> Result<ExecFlow, String> {
        match stmt {
            Stmt::Use { specifier, alias } => {
                let imported = self.import_module(specifier, module_dir)?;
                Env::define(env, alias.clone(), imported);
                Ok(ExecFlow::Continue)
            }
            Stmt::Let { name, expr } => {
                let value = self.eval_expr(expr, env, module_dir)?;
                Env::define(env, name.clone(), value);
                Ok(ExecFlow::Continue)
            }
            Stmt::Assign { target, expr } => {
                let value = self.eval_expr(expr, env, module_dir)?;
                self.assign_target(target, value, env)?;
                Ok(ExecFlow::Continue)
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_value = self.eval_expr(condition, env, module_dir)?;
                if is_truthy(&condition_value) {
                    self.execute_block(then_branch, env, module_dir)
                } else {
                    self.execute_block(else_branch, env, module_dir)
                }
            }
            Stmt::While { condition, body } => {
                loop {
                    let condition_value = self.eval_expr(condition, env, module_dir)?;
                    if !is_truthy(&condition_value) {
                        break;
                    }

                    match self.execute_block(body, env, module_dir)? {
                        ExecFlow::Continue => {}
                        ExecFlow::Return(value) => return Ok(ExecFlow::Return(value)),
                    }
                }
                Ok(ExecFlow::Continue)
            }
            Stmt::Repeat { count, body } => {
                let times = self.eval_expr(count, env, module_dir)?.as_number()?;
                if times < 0.0 {
                    return Err("repeat count must be >= 0".to_string());
                }

                let iterations = times.floor() as usize;
                for _ in 0..iterations {
                    match self.execute_block(body, env, module_dir)? {
                        ExecFlow::Continue => {}
                        ExecFlow::Return(value) => return Ok(ExecFlow::Return(value)),
                    }
                }
                Ok(ExecFlow::Continue)
            }
            Stmt::Return(expr) => {
                let value = self.eval_expr(expr, env, module_dir)?;
                Ok(ExecFlow::Return(value))
            }
            Stmt::Expr(expr) => {
                let _ = self.eval_expr(expr, env, module_dir)?;
                Ok(ExecFlow::Continue)
            }
        }
    }

    fn assign_target(&self, target: &[String], value: Value, env: &EnvRef) -> Result<(), String> {
        if target.is_empty() {
            return Err("Invalid assignment target".to_string());
        }

        if target.len() == 1 {
            let name = &target[0];
            if !Env::assign(env, name, value.clone()) {
                Env::define(env, name.clone(), value);
            }
            return Ok(());
        }

        let mut current = Env::get(env, &target[0])
            .ok_or_else(|| format!("Undefined variable '{}'", target[0]))?
            .as_object()?;

        for prop in &target[1..target.len() - 1] {
            let next = current
                .borrow()
                .get(prop)
                .cloned()
                .unwrap_or_else(new_object);
            let next_obj = next.as_object()?;
            current
                .borrow_mut()
                .insert(prop.clone(), Value::Object(next_obj.clone()));
            current = next_obj;
        }

        let last = target
            .last()
            .ok_or_else(|| "Invalid assignment target".to_string())?
            .clone();
        current.borrow_mut().insert(last, value);
        Ok(())
    }

    fn import_module(&mut self, specifier: &str, module_dir: &Path) -> Result<Value, String> {
        if is_path_like(specifier) {
            let target = resolve_path(module_dir, Path::new(specifier));
            return self.load_module(&target);
        }

        if let Some(std_module) = self.stdlib.get(specifier) {
            return Ok(std_module.clone());
        }

        if let Some(package_entry) = resolve_package_entry(module_dir, specifier) {
            return self.load_module(&package_entry);
        }

        Err(format!(
            "Module '{specifier}' not found. Install it with vpm into void_modules/{specifier}"
        ))
    }

    fn eval_expr(&mut self, expr: &Expr, env: &EnvRef, module_dir: &Path) -> Result<Value, String> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::String(s) => Ok(Value::from_str(s)),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Null => Ok(Value::Null),
            Expr::Var(name) => Env::get(env, name).ok_or_else(|| format!("Undefined variable '{name}'")),
            Expr::Member { object, property } => {
                let obj = self.eval_expr(object, env, module_dir)?.as_object()?;
                obj.borrow()
                    .get(property)
                    .cloned()
                    .ok_or_else(|| format!("Missing property '{property}'"))
            }
            Expr::Call { callee, args } => {
                let function = self.eval_expr(callee, env, module_dir)?.as_function()?;
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.eval_expr(arg, env, module_dir)?);
                }
                self.call_function(function, values, module_dir)
            }
            Expr::Binary { left, op, right } => {
                if matches!(op, BinaryOp::And) {
                    let l = self.eval_expr(left, env, module_dir)?;
                    if !is_truthy(&l) {
                        return Ok(Value::Bool(false));
                    }
                    let r = self.eval_expr(right, env, module_dir)?;
                    return Ok(Value::Bool(is_truthy(&r)));
                }

                if matches!(op, BinaryOp::Or) {
                    let l = self.eval_expr(left, env, module_dir)?;
                    if is_truthy(&l) {
                        return Ok(Value::Bool(true));
                    }
                    let r = self.eval_expr(right, env, module_dir)?;
                    return Ok(Value::Bool(is_truthy(&r)));
                }

                let l = self.eval_expr(left, env, module_dir)?;
                let r = self.eval_expr(right, env, module_dir)?;
                eval_binary(*op, l, r)
            }
            Expr::Unary { op, expr } => {
                let value = self.eval_expr(expr, env, module_dir)?;
                match op {
                    UnaryOp::Neg => Ok(Value::Number(-value.as_number()?)),
                    UnaryOp::Not => Ok(Value::Bool(!is_truthy(&value))),
                }
            }
            Expr::FnLiteral { params, body } => Ok(Value::Function(Rc::new(Function::User(
                crate::value::UserFunction {
                    params: params.clone(),
                    body: body.clone(),
                    closure: env.clone(),
                },
            )))),
        }
    }

    fn call_function(
        &mut self,
        function: Rc<Function>,
        args: Vec<Value>,
        module_dir: &Path,
    ) -> Result<Value, String> {
        match function.as_ref() {
            Function::Native(func) => func(args),
            Function::User(user) => {
                let function_env = Env::new(Some(user.closure.clone()));

                for (index, param) in user.params.iter().enumerate() {
                    let value = args.get(index).cloned().unwrap_or(Value::Null);
                    Env::define(&function_env, param.clone(), value);
                }

                match self.execute_block(&user.body, &function_env, module_dir)? {
                    ExecFlow::Continue => Ok(Value::Null),
                    ExecFlow::Return(value) => Ok(value),
                }
            }
        }
    }

    fn build_stdlib(&self) -> HashMap<String, Value> {
        let mut modules = HashMap::new();

        let time = new_object();
        set_object_prop(&time, "now_ms", native_wrap(native_time_now_ms)).expect("time.now_ms");
        set_object_prop(&time, "sleep_ms", native_wrap(native_time_sleep_ms)).expect("time.sleep_ms");
        set_object_prop(&time, "iso", native_wrap(native_time_iso)).expect("time.iso");

        let fs_mod = new_object();
        set_object_prop(&fs_mod, "read_text", native_wrap(native_fs_read_text)).expect("fs.read_text");
        set_object_prop(&fs_mod, "write_text", native_wrap(native_fs_write_text)).expect("fs.write_text");
        set_object_prop(&fs_mod, "exists", native_wrap(native_fs_exists)).expect("fs.exists");
        set_object_prop(&fs_mod, "list", native_wrap(native_fs_list)).expect("fs.list");
        set_object_prop(&fs_mod, "mkdir", native_wrap(native_fs_mkdir)).expect("fs.mkdir");
        set_object_prop(&fs_mod, "mkdir_all", native_wrap(native_fs_mkdir_all)).expect("fs.mkdir_all");
        set_object_prop(&fs_mod, "remove_file", native_wrap(native_fs_remove_file)).expect("fs.remove_file");
        set_object_prop(&fs_mod, "remove_dir_all", native_wrap(native_fs_remove_dir_all))
            .expect("fs.remove_dir_all");

        let path_mod = new_object();
        set_object_prop(&path_mod, "join", native_wrap(native_path_join)).expect("path.join");
        set_object_prop(&path_mod, "dirname", native_wrap(native_path_dirname)).expect("path.dirname");
        set_object_prop(&path_mod, "basename", native_wrap(native_path_basename)).expect("path.basename");
        set_object_prop(&path_mod, "extname", native_wrap(native_path_extname)).expect("path.extname");
        set_object_prop(&path_mod, "normalize", native_wrap(native_path_normalize)).expect("path.normalize");

        let cmd_mod = new_object();
        set_object_prop(&cmd_mod, "run", native_wrap(native_cmd_run)).expect("cmd.run");
        set_object_prop(&cmd_mod, "status", native_wrap(native_cmd_status)).expect("cmd.status");

        let http_mod = new_object();
        set_object_prop(&http_mod, "get", native_wrap(native_http_get)).expect("http.get");
        set_object_prop(&http_mod, "post", native_wrap(native_http_post)).expect("http.post");

        let json_mod = new_object();
        set_object_prop(&json_mod, "parse", native_wrap(native_json_parse)).expect("json.parse");
        set_object_prop(&json_mod, "stringify", native_wrap(native_json_stringify))
            .expect("json.stringify");

        let void_mod = new_object();
        set_object_prop(&void_mod, "id", native_wrap(native_void_id)).expect("void.id");
        set_object_prop(&void_mod, "now_us", native_wrap(native_void_now_us)).expect("void.now_us");
        set_object_prop(&void_mod, "cpu_count", native_wrap(native_void_cpu_count))
            .expect("void.cpu_count");
        set_object_prop(&void_mod, "uptime_ms", native_wrap(native_void_uptime_ms))
            .expect("void.uptime_ms");
        set_object_prop(&void_mod, "rand", native_wrap(native_void_rand)).expect("void.rand");

        let process_mod = new_object();
        let args_joined = self.argv.join("\n");
        let args_for_lookup = self.argv.clone();
        set_object_prop(
            &process_mod,
            "args",
            native_closure(move |_args| Ok(Value::from_str(&args_joined))),
        )
        .expect("process.args");
        set_object_prop(
            &process_mod,
            "argc",
            native_closure(move |_args| Ok(Value::Number(args_for_lookup.len() as f64))),
        )
        .expect("process.argc");

        let args_for_index = self.argv.clone();
        set_object_prop(
            &process_mod,
            "arg",
            native_closure(move |args| {
                let index = arg_number(&args, 0, "arg(index)")? as usize;
                if let Some(value) = args_for_index.get(index) {
                    Ok(Value::from_str(value))
                } else {
                    Ok(Value::Null)
                }
            }),
        )
        .expect("process.arg");
        set_object_prop(&process_mod, "cwd", native_wrap(native_process_cwd)).expect("process.cwd");
        set_object_prop(&process_mod, "chdir", native_wrap(native_process_chdir)).expect("process.chdir");
        set_object_prop(&process_mod, "env", native_wrap(native_process_env)).expect("process.env");
        set_object_prop(&process_mod, "set_env", native_wrap(native_process_set_env)).expect("process.set_env");
        set_object_prop(&process_mod, "platform", native_wrap(native_process_platform))
            .expect("process.platform");
        set_object_prop(&process_mod, "arch", native_wrap(native_process_arch)).expect("process.arch");
        set_object_prop(&process_mod, "pid", native_wrap(native_process_pid)).expect("process.pid");
        set_object_prop(&process_mod, "exit", native_wrap(native_process_exit)).expect("process.exit");

        // Preferred package names
        modules.insert("time".to_string(), time.clone());
        modules.insert("fs".to_string(), fs_mod.clone());
        modules.insert("path".to_string(), path_mod.clone());
        modules.insert("process".to_string(), process_mod.clone());
        modules.insert("cmd".to_string(), cmd_mod.clone());
        modules.insert("http".to_string(), http_mod.clone());
        modules.insert("json".to_string(), json_mod);
        modules.insert("void".to_string(), void_mod);

        modules
    }
}

fn resolve_path(base: &Path, spec: &Path) -> PathBuf {
    let mut candidate = if spec.is_absolute() {
        spec.to_path_buf()
    } else {
        base.join(spec)
    };

    if candidate.exists() {
        return candidate;
    }

    if candidate.extension().is_none() {
        candidate.set_extension("void");
    }

    candidate
}

fn is_path_like(specifier: &str) -> bool {
    specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier.starts_with('/')
        || specifier.ends_with(".void")
}

fn resolve_package_entry(start_dir: &Path, package_name: &str) -> Option<PathBuf> {
    let mut cursor = Some(start_dir);
    while let Some(dir) = cursor {
        let module_root = dir.join("void_modules").join(package_name);
        if let Some(entry) = resolve_package_root_entry(&module_root) {
            return Some(entry);
        }
        cursor = dir.parent();
    }
    None
}

fn resolve_package_root_entry(module_root: &Path) -> Option<PathBuf> {
    if let Some(entry) = resolve_existing_module(module_root) {
        return Some(entry);
    }

    let repo_root = module_root.join("repo");
    resolve_existing_module(&repo_root)
}

fn resolve_existing_module(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }

    if path.is_dir() {
        return resolve_directory_entry(path);
    }

    if path.extension().is_none() {
        let with_ext = path.with_extension("void");
        if with_ext.is_file() {
            return Some(with_ext);
        }
    }

    None
}

fn resolve_directory_entry(dir: &Path) -> Option<PathBuf> {
    if let Some(entry) = read_manifest_entry(dir) {
        return Some(entry);
    }

    for rel in ["index.void", "main.void", "src/index.void", "src/main.void"] {
        let candidate = dir.join(rel);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn read_manifest_entry(dir: &Path) -> Option<PathBuf> {
    for manifest_name in ["void.json", "package.json"] {
        let manifest_path = dir.join(manifest_name);
        if !manifest_path.is_file() {
            continue;
        }

        let raw = match fs::read_to_string(&manifest_path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        let manifest = match serde_json::from_str::<ModuleManifest>(&raw) {
            Ok(parsed) => parsed,
            Err(_) => continue,
        };

        for target in [manifest.main, manifest.module, manifest.entry]
            .into_iter()
            .flatten()
        {
            if let Some(resolved) = resolve_manifest_target(dir, &target) {
                return Some(resolved);
            }
        }
    }

    None
}

fn resolve_manifest_target(base: &Path, target: &str) -> Option<PathBuf> {
    let target = target.trim();
    if target.is_empty() {
        return None;
    }

    let candidate = if Path::new(target).is_absolute() {
        PathBuf::from(target)
    } else {
        base.join(target)
    };

    resolve_existing_module(&candidate)
}

fn set_object_prop(object: &Value, key: &str, value: Value) -> Result<(), String> {
    let obj = object.as_object()?;
    obj.borrow_mut().insert(key.to_string(), value);
    Ok(())
}

fn native_wrap(func: fn(Vec<Value>) -> Result<Value, String>) -> Value {
    native_closure(move |args| func(args))
}

fn native_closure<F>(func: F) -> Value
where
    F: Fn(Vec<Value>) -> Result<Value, String> + 'static,
{
    Value::Function(Rc::new(Function::Native(Rc::new(func))))
}

fn eval_binary(op: BinaryOp, left: Value, right: Value) -> Result<Value, String> {
    match op {
        BinaryOp::Add => match (left, right) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
            (a, b) => Ok(Value::from_str(&(a.to_text() + &b.to_text()))),
        },
        BinaryOp::Sub => Ok(Value::Number(left.as_number()? - right.as_number()?)),
        BinaryOp::Mul => Ok(Value::Number(left.as_number()? * right.as_number()?)),
        BinaryOp::Div => Ok(Value::Number(left.as_number()? / right.as_number()?)),
        BinaryOp::Eq => Ok(Value::Bool(value_equals(&left, &right))),
        BinaryOp::Ne => Ok(Value::Bool(!value_equals(&left, &right))),
        BinaryOp::Lt => compare_values(&left, &right, |a, b| a < b),
        BinaryOp::Lte => compare_values(&left, &right, |a, b| a <= b),
        BinaryOp::Gt => compare_values(&left, &right, |a, b| a > b),
        BinaryOp::Gte => compare_values(&left, &right, |a, b| a >= b),
        BinaryOp::And | BinaryOp::Or => unreachable!(),
    }
}

fn compare_values<F>(left: &Value, right: &Value, cmp: F) -> Result<Value, String>
where
    F: Fn(f64, f64) -> bool,
{
    Ok(Value::Bool(cmp(left.as_number()?, right.as_number()?)))
}

fn value_equals(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => (*a - *b).abs() < f64::EPSILON,
        (Value::String(a), Value::String(b)) => a == b,
        _ => false,
    }
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => *v != 0.0,
        Value::String(v) => !v.is_empty(),
        Value::Object(_) | Value::Function(_) => true,
    }
}

fn native_say(args: Vec<Value>) -> Result<Value, String> {
    let mut out = String::new();
    for (index, value) in args.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(&value.to_text());
    }
    println!("{out}");
    Ok(Value::Null)
}

fn native_console_error(args: Vec<Value>) -> Result<Value, String> {
    let mut out = String::new();
    for (index, value) in args.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(&value.to_text());
    }
    eprintln!("{out}");
    Ok(Value::Null)
}

fn native_time_now_ms(_args: Vec<Value>) -> Result<Value, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    Ok(Value::Number(now.as_millis() as f64))
}

fn native_time_sleep_ms(args: Vec<Value>) -> Result<Value, String> {
    let millis = arg_number(&args, 0, "sleep_ms(ms)")?;
    if millis < 0.0 {
        return Err("sleep_ms(ms) requires ms >= 0".to_string());
    }

    thread::sleep(Duration::from_millis(millis as u64));
    Ok(Value::Null)
}

fn native_time_iso(_args: Vec<Value>) -> Result<Value, String> {
    Ok(Value::from_str(&chrono_like_iso_now()))
}

fn native_fs_read_text(args: Vec<Value>) -> Result<Value, String> {
    let path = arg_string(&args, 0, "read_text(path)")?;
    let data = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(Value::from_str(&data))
}

fn native_fs_write_text(args: Vec<Value>) -> Result<Value, String> {
    let path = arg_string(&args, 0, "write_text(path, text)")?;
    let text = arg_string(&args, 1, "write_text(path, text)")?;
    fs::write(path, text).map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

fn native_fs_exists(args: Vec<Value>) -> Result<Value, String> {
    let path = arg_string(&args, 0, "exists(path)")?;
    Ok(Value::Bool(Path::new(path).exists()))
}

fn native_fs_list(args: Vec<Value>) -> Result<Value, String> {
    let dir = if args.is_empty() {
        "."
    } else {
        arg_string(&args, 0, "list(path)")?
    };

    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        entries.push(entry.file_name().to_string_lossy().to_string());
    }
    entries.sort();
    Ok(Value::from_str(&entries.join("\n")))
}

fn native_fs_mkdir(args: Vec<Value>) -> Result<Value, String> {
    let path = arg_string(&args, 0, "mkdir(path)")?;
    fs::create_dir(path).map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

fn native_fs_mkdir_all(args: Vec<Value>) -> Result<Value, String> {
    let path = arg_string(&args, 0, "mkdir_all(path)")?;
    fs::create_dir_all(path).map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

fn native_fs_remove_file(args: Vec<Value>) -> Result<Value, String> {
    let path = arg_string(&args, 0, "remove_file(path)")?;
    fs::remove_file(path).map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

fn native_fs_remove_dir_all(args: Vec<Value>) -> Result<Value, String> {
    let path = arg_string(&args, 0, "remove_dir_all(path)")?;
    fs::remove_dir_all(path).map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

fn native_path_join(args: Vec<Value>) -> Result<Value, String> {
    if args.is_empty() {
        return Err("join(...) expects at least one part".to_string());
    }

    let mut out = PathBuf::new();
    for arg in args {
        out.push(arg.as_string()?);
    }
    Ok(Value::from_str(&out.display().to_string()))
}

fn native_path_dirname(args: Vec<Value>) -> Result<Value, String> {
    let path = PathBuf::from(arg_string(&args, 0, "dirname(path)")?);
    if let Some(parent) = path.parent() {
        return Ok(Value::from_str(&parent.display().to_string()));
    }
    Ok(Value::from_str(""))
}

fn native_path_basename(args: Vec<Value>) -> Result<Value, String> {
    let path = PathBuf::from(arg_string(&args, 0, "basename(path)")?);
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    Ok(Value::from_str(name))
}

fn native_path_extname(args: Vec<Value>) -> Result<Value, String> {
    let path = PathBuf::from(arg_string(&args, 0, "extname(path)")?);
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    Ok(Value::from_str(ext))
}

fn native_path_normalize(args: Vec<Value>) -> Result<Value, String> {
    let path = PathBuf::from(arg_string(&args, 0, "normalize(path)")?);
    Ok(Value::from_str(&path.display().to_string()))
}

fn native_process_cwd(_args: Vec<Value>) -> Result<Value, String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    Ok(Value::from_str(&cwd.display().to_string()))
}

fn native_process_chdir(args: Vec<Value>) -> Result<Value, String> {
    let path = arg_string(&args, 0, "chdir(path)")?;
    std::env::set_current_dir(path).map_err(|e| e.to_string())?;
    Ok(Value::Null)
}

fn native_process_env(args: Vec<Value>) -> Result<Value, String> {
    let key = arg_string(&args, 0, "env(key)")?;
    match std::env::var(key) {
        Ok(v) => Ok(Value::from_str(&v)),
        Err(_) => Ok(Value::Null),
    }
}

fn native_process_set_env(args: Vec<Value>) -> Result<Value, String> {
    let key = arg_string(&args, 0, "set_env(key, value)")?;
    let value = arg_string(&args, 1, "set_env(key, value)")?;
    unsafe {
        std::env::set_var(key, value);
    }
    Ok(Value::Null)
}

fn native_process_platform(_args: Vec<Value>) -> Result<Value, String> {
    Ok(Value::from_str(std::env::consts::OS))
}

fn native_process_arch(_args: Vec<Value>) -> Result<Value, String> {
    Ok(Value::from_str(std::env::consts::ARCH))
}

fn native_process_pid(_args: Vec<Value>) -> Result<Value, String> {
    Ok(Value::Number(std::process::id() as f64))
}

fn native_process_exit(args: Vec<Value>) -> Result<Value, String> {
    let code = if args.is_empty() {
        0
    } else {
        arg_number(&args, 0, "exit(code)")? as i32
    };
    std::process::exit(code);
}

fn native_cmd_run(args: Vec<Value>) -> Result<Value, String> {
    let command = arg_string(&args, 0, "cmd.run(command)")?;
    let output = run_shell(command)?;
    Ok(Value::from_str(output.trim_end()))
}

fn native_cmd_status(args: Vec<Value>) -> Result<Value, String> {
    let command = arg_string(&args, 0, "cmd.status(command)")?;
    let status = run_shell_status(command)?;
    Ok(Value::Number(status as f64))
}

fn native_http_get(args: Vec<Value>) -> Result<Value, String> {
    let url = arg_string(&args, 0, "http.get(url)")?;
    let response = reqwest::blocking::get(url)
        .map_err(|e| format!("HTTP get failed: {e}"))?
        .text()
        .map_err(|e| format!("HTTP body read failed: {e}"))?;
    Ok(Value::from_str(&response))
}

fn native_http_post(args: Vec<Value>) -> Result<Value, String> {
    let url = arg_string(&args, 0, "http.post(url, body)")?;
    let body = arg_string(&args, 1, "http.post(url, body)")?;
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(url)
        .body(body.to_string())
        .send()
        .map_err(|e| format!("HTTP post failed: {e}"))?
        .text()
        .map_err(|e| format!("HTTP body read failed: {e}"))?;
    Ok(Value::from_str(&response))
}

fn native_json_parse(args: Vec<Value>) -> Result<Value, String> {
    let raw = arg_string(&args, 0, "json.parse(text)")?;
    let parsed: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("json.parse failed: {e}"))?;
    json_to_value(&parsed)
}

fn native_json_stringify(args: Vec<Value>) -> Result<Value, String> {
    let value = args.first().cloned().unwrap_or(Value::Null);
    let json = value_to_json(&value)?;
    let out = serde_json::to_string(&json).map_err(|e| format!("json.stringify failed: {e}"))?;
    Ok(Value::from_str(&out))
}

fn native_void_id(args: Vec<Value>) -> Result<Value, String> {
    let prefix = if args.is_empty() {
        "void"
    } else {
        arg_string(&args, 0, "void.id(prefix)")?
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis();
    let sequence = VOID_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(Value::from_str(&format!("{prefix}_{now}_{sequence}")))
}

fn native_void_now_us(_args: Vec<Value>) -> Result<Value, String> {
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_micros() as f64;
    Ok(Value::Number(micros))
}

fn native_void_cpu_count(_args: Vec<Value>) -> Result<Value, String> {
    let cpus = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    Ok(Value::Number(cpus as f64))
}

fn native_void_uptime_ms(_args: Vec<Value>) -> Result<Value, String> {
    let start = VOID_START.get_or_init(Instant::now);
    Ok(Value::Number(start.elapsed().as_millis() as f64))
}

fn native_void_rand(args: Vec<Value>) -> Result<Value, String> {
    if args.is_empty() {
        let value = pseudo_random_u64() as f64 / u64::MAX as f64;
        return Ok(Value::Number(value));
    }

    if args.len() == 2 {
        let min = arg_number(&args, 0, "void.rand(min, max)")?;
        let max = arg_number(&args, 1, "void.rand(min, max)")?;
        if max <= min {
            return Err("void.rand(min, max) requires max > min".to_string());
        }
        let unit = pseudo_random_u64() as f64 / u64::MAX as f64;
        return Ok(Value::Number(min + unit * (max - min)));
    }

    Err("void.rand() accepts 0 args or 2 args".to_string())
}

fn arg_string<'a>(args: &'a [Value], index: usize, signature: &str) -> Result<&'a str, String> {
    args.get(index)
        .ok_or_else(|| format!("{signature} missing argument at index {index}"))?
        .as_string()
}

fn arg_number(args: &[Value], index: usize, signature: &str) -> Result<f64, String> {
    args.get(index)
        .ok_or_else(|| format!("{signature} missing argument at index {index}"))?
        .as_number()
}

fn run_shell(command: &str) -> Result<String, String> {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .arg("/C")
            .arg(command)
            .output()
            .map_err(|e| e.to_string())?
    } else {
        Command::new("sh")
            .arg("-lc")
            .arg(command)
            .output()
            .map_err(|e| e.to_string())?
    };

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_shell_status(command: &str) -> Result<i32, String> {
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .arg("/C")
            .arg(command)
            .status()
            .map_err(|e| e.to_string())?
    } else {
        Command::new("sh")
            .arg("-lc")
            .arg(command)
            .status()
            .map_err(|e| e.to_string())?
    };

    Ok(status.code().unwrap_or(1))
}

fn json_to_value(value: &serde_json::Value) -> Result<Value, String> {
    match value {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(v) => Ok(Value::Bool(*v)),
        serde_json::Value::Number(n) => n
            .as_f64()
            .map(Value::Number)
            .ok_or_else(|| "json.parse encountered unsupported number".to_string()),
        serde_json::Value::String(s) => Ok(Value::from_str(s)),
        serde_json::Value::Array(items) => {
            let obj = new_object().as_object()?;
            for (index, item) in items.iter().enumerate() {
                obj.borrow_mut().insert(index.to_string(), json_to_value(item)?);
            }
            obj.borrow_mut()
                .insert("length".to_string(), Value::Number(items.len() as f64));
            Ok(Value::Object(obj))
        }
        serde_json::Value::Object(entries) => {
            let obj = new_object().as_object()?;
            for (key, value) in entries {
                obj.borrow_mut().insert(key.clone(), json_to_value(value)?);
            }
            Ok(Value::Object(obj))
        }
    }
}

fn value_to_json(value: &Value) -> Result<serde_json::Value, String> {
    match value {
        Value::Null => Ok(serde_json::Value::Null),
        Value::Bool(v) => Ok(serde_json::Value::Bool(*v)),
        Value::Number(v) => serde_json::Number::from_f64(*v)
            .map(serde_json::Value::Number)
            .ok_or_else(|| "json.stringify cannot encode NaN/Infinity".to_string()),
        Value::String(v) => Ok(serde_json::Value::String(v.to_string())),
        Value::Function(_) => Err("json.stringify cannot encode functions".to_string()),
        Value::Object(obj) => {
            let borrowed = obj.borrow();
            if let Some(arr) = object_to_json_array(&borrowed)? {
                return Ok(serde_json::Value::Array(arr));
            }

            let mut map = serde_json::Map::new();
            for (key, value) in borrowed.iter() {
                map.insert(key.clone(), value_to_json(value)?);
            }
            Ok(serde_json::Value::Object(map))
        }
    }
}

fn object_to_json_array(values: &HashMap<String, Value>) -> Result<Option<Vec<serde_json::Value>>, String> {
    let Some(length_value) = values.get("length") else {
        return Ok(None);
    };

    let length = match length_value {
        Value::Number(n) if *n >= 0.0 && n.fract() == 0.0 => *n as usize,
        _ => return Ok(None),
    };

    if values.keys().any(|key| {
        if key == "length" {
            return false;
        }
        match key.parse::<usize>() {
            Ok(idx) => idx >= length,
            Err(_) => true,
        }
    }) {
        return Ok(None);
    }

    let mut out = Vec::with_capacity(length);
    for index in 0..length {
        if let Some(value) = values.get(&index.to_string()) {
            out.push(value_to_json(value)?);
        } else {
            out.push(serde_json::Value::Null);
        }
    }

    Ok(Some(out))
}

fn pseudo_random_u64() -> u64 {
    let mut seed = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as u64,
        Err(_) => 0,
    };
    seed ^= VOID_ID_COUNTER.fetch_add(1, Ordering::Relaxed);

    // xorshift64*
    seed ^= seed >> 12;
    seed ^= seed << 25;
    seed ^= seed >> 27;
    seed.wrapping_mul(2685821657736338717)
}

fn chrono_like_iso_now() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => format!("{}", d.as_secs()),
        Err(_) => "0".to_string(),
    }
}
