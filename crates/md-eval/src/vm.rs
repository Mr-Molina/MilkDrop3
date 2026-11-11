//! Variable table and memory owned by one expression VM.

use crate::mem::{GlobalMem, MegaBuf};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// `NSEEL_MAX_VARIABLE_NAMELEN` (ns-eel.h): names compare on this many
/// characters, so longer names are truncated to it.
pub const MAX_VARIABLE_NAMELEN: usize = 16;

/// Registers are numbered from here, like `NSEEL_GLOBALVAR_BASE` (1 << 24).
const REG_BASE: u32 = 1 << 24;

/// Handle to a variable in a `Vm`'s table, or to a shared register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VarRef(u32);

impl VarRef {
    /// Raw handle value (registers start at `1 << 24`).
    pub fn index(self) -> u32 {
        self.0
    }
}

/// Variables, `megabuf`, and a handle to the shared registers and `gmegabuf`.
pub struct Vm {
    names: HashMap<String, u32>,
    values: Vec<f64>,
    pub megabuf: MegaBuf,
    pub global: Rc<RefCell<GlobalMem>>,
}

/// Register number when `name` (already lowercase) is exactly `reg` + 2 digits.
fn reg_index(name: &str) -> Option<u32> {
    let d = name.strip_prefix("reg")?.as_bytes();
    (d.len() == 2 && d.iter().all(u8::is_ascii_digit))
        .then(|| u32::from(d[0] - b'0') * 10 + u32::from(d[1] - b'0'))
}

impl Vm {
    pub fn new(global: Rc<RefCell<GlobalMem>>) -> Vm {
        Vm {
            names: HashMap::new(),
            values: Vec::new(),
            megabuf: MegaBuf::new(),
            global,
        }
    }

    /// Look up or create (at 0.0) a variable. Case-insensitive; only the
    /// first 16 characters matter. `regNN` (exactly two digits) is a shared
    /// register.
    pub fn var(&mut self, name: &str) -> VarRef {
        let key: String = name
            .chars()
            .map(|c| c.to_ascii_lowercase())
            .take(MAX_VARIABLE_NAMELEN)
            .collect();
        if let Some(n) = reg_index(&key) {
            return VarRef(REG_BASE + n);
        }
        if let Some(&i) = self.names.get(&key) {
            return VarRef(i);
        }
        let i = self.values.len() as u32;
        self.values.push(0.0);
        self.names.insert(key, i);
        VarRef(i)
    }

    pub fn get(&self, v: VarRef) -> f64 {
        if v.0 >= REG_BASE {
            self.global.borrow().regs[(v.0 - REG_BASE) as usize]
        } else {
            self.values[v.0 as usize]
        }
    }

    pub fn set(&mut self, v: VarRef, x: f64) {
        if v.0 >= REG_BASE {
            self.global.borrow_mut().regs[(v.0 - REG_BASE) as usize] = x;
        } else {
            self.values[v.0 as usize] = x;
        }
    }

    /// Zero every local variable and free `megabuf`. The shared registers
    /// and `gmegabuf` belong to `GlobalMem` and are left alone.
    pub fn reset_vars(&mut self) {
        self.values.fill(0.0);
        self.megabuf.clear();
    }
}
