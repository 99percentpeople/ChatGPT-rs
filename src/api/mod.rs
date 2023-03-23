use std::cell::Cell;

pub mod chat;
pub mod complete;
pub mod models;

#[derive(Debug, Clone, Copy)]
pub enum ParameterRange {
    Number(f32, f32),
    Integer(u32, u32),
}

impl From<(f32, f32)> for ParameterRange {
    fn from(value: (f32, f32)) -> Self {
        Self::Number(value.0, value.1)
    }
}
impl From<(u32, u32)> for ParameterRange {
    fn from(value: (u32, u32)) -> Self {
        Self::Integer(value.0, value.1)
    }
}
#[derive(Debug, Clone, Copy)]
pub enum ParameterValue {
    Number(f32),
    Integer(u32),
    OptionalNumber(Option<f32>),
    OptionalInteger(Option<u32>),
}

impl From<f32> for ParameterValue {
    fn from(value: f32) -> Self {
        Self::Number(value)
    }
}

impl From<u32> for ParameterValue {
    fn from(value: u32) -> Self {
        Self::Integer(value)
    }
}

impl From<Option<f32>> for ParameterValue {
    fn from(value: Option<f32>) -> Self {
        Self::OptionalNumber(value)
    }
}

impl From<Option<u32>> for ParameterValue {
    fn from(value: Option<u32>) -> Self {
        Self::OptionalInteger(value)
    }
}

pub trait Parameter {
    fn name(&self) -> &'static str;
    fn range(&self) -> ParameterRange;
    fn default(&self) -> ParameterValue;
    fn store(&self) -> ParameterValue;
    fn set(&self, value: ParameterValue);
    fn get(&self) -> ParameterValue;
}

pub struct Param<T: Sized> {
    name: &'static str,
    range: ParameterRange,
    default: ParameterValue,
    store: Cell<T>,
    getter: Box<dyn Fn() -> T>,
    setter: Box<dyn Fn(T)>,
}

default impl<T> Parameter for Param<T> {
    fn range(&self) -> ParameterRange {
        self.range
    }

    fn name(&self) -> &'static str {
        self.name
    }
    fn default(&self) -> ParameterValue {
        self.default
    }
    fn store(&self) -> ParameterValue {
        self.default()
    }
}

impl Parameter for Param<u32> {
    fn set(&self, value: ParameterValue) {
        if let ParameterValue::Integer(value) = value {
            self.setter.call((value,));
        }
    }

    fn get(&self) -> ParameterValue {
        ParameterValue::Integer(self.getter.call(()))
    }
}

impl Parameter for Param<f32> {
    fn set(&self, value: ParameterValue) {
        if let ParameterValue::Number(value) = value {
            self.setter.call((value,));
        }
    }

    fn get(&self) -> ParameterValue {
        ParameterValue::Number(self.getter.call(()))
    }
}

impl Parameter for Param<Option<u32>> {
    fn set(&self, value: ParameterValue) {
        if let ParameterValue::OptionalInteger(value) = value {
            self.setter.call((value,));
            if let Some(value) = value {
                self.store.replace(Some(value));
            }
        }
    }

    fn get(&self) -> ParameterValue {
        ParameterValue::OptionalInteger(self.getter.call(()))
    }

    fn store(&self) -> ParameterValue {
        if let Some(store) = self.store.get() {
            ParameterValue::Integer(store)
        } else {
            self.default()
        }
    }
}

pub trait ParameterControl {
    fn params(&self) -> Vec<Box<dyn Parameter>>;
}
