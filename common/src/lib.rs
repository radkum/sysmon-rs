#![no_std]

#[repr(C)]
#[derive(Debug)]
pub enum ItemInfo {
    ProcessCreate {
        pid: u32,
        parent_pid: u32,
        //command_line: String,
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
        //image_file_name: String,
    },
    RegistrySetValue {
        pid: u32,
        tid: u32,
        //key_name: String,
        //value_name: String,
        data_type: u32,
        //data: Vec<u8>,
    },
}