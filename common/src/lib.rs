#![no_std]
extern crate alloc;

use core::fmt::Formatter;
use alloc::string::String;

pub const BUFF_SIZE: usize = 64;
pub struct StringBuff([u8;BUFF_SIZE]);

#[repr(C)]
#[derive(Debug)]
pub enum ItemInfo {
    ProcessCreate {
        pid: u32,
        parent_pid: u32,
        command_line: StringBuff,
    },
    ProcessExit {
        pid: u32,
    },
    ThreadCreate {
        pid: u32,
        tid: u32,
    },
    ThreadExit {
        pid: u32,
        tid: u32,
    },
    ImageLoad {
        pid: u32,
        load_address: isize,
        image_size: usize,
        image_file_name: StringBuff,
    },
    RegistrySetValue {
        pid: u32,
        tid: u32,
        key_name: StringBuff,
        //value_name: String,
        data_type: u32,
        //data: Vec<u8>,
    },
}

impl core::fmt::Debug for StringBuff {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let v = self.0;
        let index = v.iter().position(|&r| r == 0).unwrap_or_default();
        let (v, _) = v.split_at(index);
        let s = match String::from_utf8(v.to_vec()) {
            Ok(v) => v,
            Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
        };
        let s = s.trim_end();
        write!(f, "\"{s}\"")
    }
}

impl ItemInfo {
    pub fn string_to_buffer(s: String) -> StringBuff {
        let mut buff = StringBuff([0u8; BUFF_SIZE]);

        let size_to_copy = if s.len() > BUFF_SIZE - 1 {
            BUFF_SIZE - 1
        } else {
            s.len()
        };
        unsafe {
            core::ptr::copy_nonoverlapping(s.as_ptr(), buff.0.as_mut_ptr(), size_to_copy);
        }

        buff
    }
}
