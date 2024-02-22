use crate::error_msg::print_last_error;
use common::ItemInfo;
use std::{mem::size_of, ptr::null_mut};
use windows::core::imp::HANDLE;
use windows_sys::Win32::{
    Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{CreateFileA, ReadFile, OPEN_EXISTING},
    System::Threading::Sleep,
};

mod error_msg;

fn main() {
    println!("Hello, world!");

    unsafe {
        let h_file = CreateFileA(
            "\\\\.\\SysMon\0".as_ptr(),
            GENERIC_READ,
            0,
            null_mut(),
            OPEN_EXISTING,
            0,
            0isize,
        ) as HANDLE;
        if h_file == INVALID_HANDLE_VALUE {
            print_last_error("Failed to open file");
            return;
        }
        println!("CreateFile success!");

        let mut buffer = [0u8; 0x10000];

        //loop {
        let mut bytes: u32 = 0;
        let status = ReadFile(
            h_file,
            buffer.as_mut_ptr(),
            std::mem::size_of_val(&buffer) as u32,
            &mut bytes as *mut u32,
            null_mut(),
        );
        if status == 0 {
            print_last_error("Failed to open file");
            return;
        }
        println!("Read success! Bytes: {bytes}");
        if bytes != 0 {
            display_info(&buffer, bytes);
        }

        //Sleep(200);
        //}
    }
}

fn display_info(buffer: &[u8], size: u32) {
    let mut offset = 0;
    loop {
        if size == offset as u32 {
            break;
        }

        let item = unsafe { &*(buffer.as_ptr().offset(offset as isize) as *const ItemInfo) };

        println!("{item:?}");
        println!("{offset}");
        offset += size_of::<ItemInfo>();
    }
}
