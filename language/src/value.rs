use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::ast::Stmt;

pub type ObjectRef = Rc<RefCell<HashMap<String, Value>>>;
pub type EnvRef = Rc<RefCell<Env>>;
pub type NativeFunction = Rc<dyn Fn(Vec<Value>) -> Result<Value, String>>;

#[derive(Clone)]
pub enum Value {
    Number(f64),
    String(Rc<str>),
    Bool(bool),
    Object(ObjectRef),
    Function(Rc<Function>),
    Null,
}

#[derive(Clone)]
pub enum Function {
    Native(NativeFunction),
    User(UserFunction),
}

#[derive(Clone)]
pub struct UserFunction {
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
    pub closure: EnvRef,
}

pub struct Env {
    values: HashMap<String, Value>,
    parent: Option<EnvRef>,
}

impl Env {
    pub fn new(parent: Option<EnvRef>) -> EnvRef {
        Rc::new(RefCell::new(Self {
            values: HashMap::new(),
            parent,
        }))
    }

    pub fn define(env: &EnvRef, name: impl Into<String>, value: Value) {
        env.borrow_mut().values.insert(name.into(), value);
    }

    pub fn assign(env: &EnvRef, name: &str, value: Value) -> bool {
        if env.borrow().values.contains_key(name) {
            env.borrow_mut().values.insert(name.to_owned(), value);
            return true;
        }

        let parent = env.borrow().parent.clone();
        if let Some(parent) = parent {
            return Self::assign(&parent, name, value);
        }

        false
    }

    pub fn get(env: &EnvRef, name: &str) -> Option<Value> {
        if let Some(value) = env.borrow().values.get(name) {
            return Some(value.clone());
        }

        let parent = env.borrow().parent.clone();
        if let Some(parent) = parent {
            return Self::get(&parent, name);
        }

        None
    }
}

impl Value {
    pub fn as_number(&self) -> Result<f64, String> {
        match self {
            Value::Number(n) => Ok(*n),
            _ => Err("Expected number".to_string()),
        }
    }

    pub fn as_string(&self) -> Result<&str, String> {
        match self {
            Value::String(s) => Ok(s),
            _ => Err("Expected string".to_string()),
        }
    }

    pub fn as_object(&self) -> Result<ObjectRef, String> {
        match self {
            Value::Object(obj) => Ok(obj.clone()),
            _ => Err("Expected object".to_string()),
        }
    }

    pub fn as_function(&self) -> Result<Rc<Function>, String> {
        match self {
            Value::Function(f) => Ok(f.clone()),
            _ => Err("Expected function".to_string()),
        }
    }

    pub fn to_text(&self) -> String {
        match self {
            Value::Number(n) => {
                if (n.fract()).abs() < f64::EPSILON {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            Value::String(s) => s.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Object(_) => "[object]".to_string(),
            Value::Function(_) => "[function]".to_string(),
            Value::Null => "null".to_string(),
        }
    }

    pub fn from_str(s: &str) -> Self {
        Value::String(Rc::<str>::from(s))
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_text())
    }
}

pub fn new_object() -> Value {
    Value::Object(Rc::new(RefCell::new(HashMap::new())))
}
