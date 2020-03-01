use crate::{
    value::{self, rf::JsObjectRef},
    Ctx, FromJs, Result, ToJs, Value,
};
use rquickjs_sys as qjs;
use std::mem;

/// Rust representation of a javascript object.
#[derive(Debug, PartialEq, Clone)]
pub struct Object<'js>(pub(crate) JsObjectRef<'js>);

impl<'js> Object<'js> {
    // Unsafe because pointers must be valid and the
    // liftime of this object must be constrained
    // Further more the JSValue must also be of type object as indicated by JS_TAG_OBJECT
    // All save functions rely on this constrained to be save
    pub(crate) unsafe fn from_js_value(ctx: Ctx<'js>, v: qjs::JSValue) -> Self {
        Object(JsObjectRef::from_js_value(ctx, v))
    }

    // Save because using the JSValue is unsafe
    pub(crate) fn as_js_value(&self) -> qjs::JSValue {
        self.0.as_js_value()
    }

    /// Create a new javascript object
    pub fn new(ctx: Ctx<'js>) -> Result<Self> {
        unsafe {
            let val = qjs::JS_NewObject(ctx.ctx);
            let val = value::handle_exception(ctx, val)?;
            Ok(Self::from_js_value(ctx, val))
        }
    }

    /// Get a new value
    pub fn get<K: ToJs<'js>, V: FromJs<'js>>(&self, k: K) -> Result<V> {
        let key = k.to_js(self.0.ctx)?;
        unsafe {
            let val = match key {
                Value::Int(x) => {
                    // TODO is this correct. Integers are signed and the index here is unsigned
                    // Soo...
                    qjs::JS_GetPropertyUint32(self.0.ctx.ctx, self.as_js_value(), x as u32)
                }
                x => {
                    let atom = qjs::JS_ValueToAtom(self.0.ctx.ctx, x.as_js_value());
                    qjs::JS_GetProperty(self.0.ctx.ctx, self.as_js_value(), atom)
                }
            };
            V::from_js(self.0.ctx, Value::from_js_value(self.0.ctx, val)?)
        }
    }

    /// check wether the object contains a certain key.
    pub fn contains_key<K>(&self, k: K) -> Result<bool>
    where
        K: ToJs<'js>,
    {
        let key = k.to_js(self.0.ctx)?;
        unsafe {
            let atom = qjs::JS_ValueToAtom(self.0.ctx.ctx, key.as_js_value());
            let res = qjs::JS_HasProperty(self.0.ctx.ctx, self.as_js_value(), atom);
            if res < 0 {
                return Err(value::get_exception(self.0.ctx));
            }
            Ok(res == 1)
        }
    }

    // TODO implement ToKey, which will create a atom for a value,
    // This can allow code to do checks for the same value faster by
    // pre computing the atom for the key.
    /// Set a member of an object to a certain value
    pub fn set<K: ToJs<'js>, V: ToJs<'js>>(&self, key: K, value: V) -> Result<()> {
        let key = key.to_js(self.0.ctx)?;
        let val = value.to_js(self.0.ctx)?;
        unsafe {
            let atom = qjs::JS_ValueToAtom(self.0.ctx.ctx, key.as_js_value());
            if qjs::JS_SetProperty(self.0.ctx.ctx, self.as_js_value(), atom, val.as_js_value()) < 0
            {
                return Err(value::get_exception(self.0.ctx));
            }
            // When we pass in the value to SetProperty, it takes ownership
            // so we should not decrement the reference count when our version drops
            mem::forget(val);
        }
        Ok(())
    }

    /// Remove a member of this objects
    pub fn remove<K: ToJs<'js>>(&self, key: K) -> Result<()> {
        let key = key.to_js(self.0.ctx)?;
        unsafe {
            let atom = qjs::JS_ValueToAtom(self.0.ctx.ctx, key.as_js_value());
            if qjs::JS_DeleteProperty(
                self.0.ctx.ctx,
                self.as_js_value(),
                atom,
                qjs::JS_PROP_THROW as i32,
            ) < 0
            {
                return Err(value::get_exception(self.0.ctx));
            }
        }
        Ok(())
    }

    pub fn is_function(&self) -> bool {
        unsafe { qjs::JS_IsFunction(self.0.ctx.ctx, self.as_js_value()) != 0 }
    }

    pub fn is_array(&self) -> bool {
        unsafe { qjs::JS_IsArray(self.0.ctx.ctx, self.as_js_value()) != 0 }
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use std::string::String as StdString;
    #[test]
    fn from_javascript() {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| {
            let val = ctx.eval::<Value, _>(
                r#"
                let obj = {};
                obj['a'] = 3;
                obj[3] = 'a';
                obj
            "#,
            );
            if let Ok(Value::Object(x)) = val {
                let text: StdString = x.get(Value::Int(3)).unwrap();
                assert_eq!(text.as_str(), "a");
                let int: i32 = x.get("a").unwrap();
                assert_eq!(int, 3);
                let int: StdString = x.get("a").unwrap();
                assert_eq!(int, "3");
                x.set("hallo", "foo").unwrap();
                assert_eq!(x.get("hallo"), Ok("foo".to_string()));
                x.remove("hallo").unwrap();
                assert_eq!(x.get("hallo"), Ok(Value::Undefined))
            } else {
                panic!();
            };
        });
    }
}
