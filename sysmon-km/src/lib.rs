#![no_std]
#![allow(non_snake_case)]
#![allow(static_mut_refs)]
extern crate alloc;

mod cleaner;
mod ioctl_code;

/// kernel-init deliver a few elements (eg. panic implementation) necessary to run code in kernel
#[allow(unused_imports)]
use kernel_init;

use crate::cleaner::Cleaner;

use common::ItemInfo::{
    self, ImageLoad, ProcessCreate, ProcessExit, RegistrySetValue, ThreadCreate, ThreadExit,
};

use kernel_fast_mutex::{auto_lock::AutoLock, fast_mutex::FastMutex, locker::Locker};
use kernel_log::KernelLogger;
use kernel_macros::{HandleToU32, NT_SUCCESS};
use kernel_string::{PUNICODE_STRING, UNICODE_STRING};

use km_api_sys::{
    ntddk::{
        PsGetCurrentProcessId, PsGetCurrentThreadId, PsRemoveCreateThreadNotifyRoutine,
        PsRemoveLoadImageNotifyRoutine, PsSetCreateProcessNotifyRoutineEx,
        PsSetCreateThreadNotifyRoutine, PsSetLoadImageNotifyRoutine, PIMAGE_INFO,
        PPS_CREATE_NOTIFY_INFO, PS_CREATE_NOTIFY_INFO, REG_NT_POST_SET_VALUE_KEY,
    },
    wmd::{
        CmCallbackGetKeyObjectIDEx, CmCallbackReleaseKeyObjectIDEx, CmRegisterCallbackEx,
        CmUnRegisterCallback, MmGetSystemAddressForMdlSafe, LARGE_INTEGER, MDL,
        PREG_POST_OPERATION_INFORMATION, PREG_SET_VALUE_KEY_INFORMATION,
    },
};

use alloc::{
    //collections::VecDeque,
    string::ToString,
    vec::Vec,
};
use core::{
    mem::{forget, size_of},
    ptr,
    ptr::null_mut,
};
use log::LevelFilter;
use winapi::{
    km::wdm::{
        IoCompleteRequest, IoCreateDevice, IoCreateSymbolicLink, IoDeleteDevice,
        IoDeleteSymbolicLink, IoGetCurrentIrpStackLocation, DEVICE_FLAGS::DO_DIRECT_IO,
        DEVICE_OBJECT, DEVICE_TYPE, DRIVER_OBJECT, IO_STACK_LOCATION, IRP, IRP_MJ, PDEVICE_OBJECT,
        PEPROCESS, _IO_STACK_LOCATION_READ,
    },
    shared::{
        ntdef::{BOOLEAN, FALSE, HANDLE, NTSTATUS, PVOID, TRUE},
        ntstatus::{
            STATUS_INSUFFICIENT_RESOURCES, STATUS_INVALID_BUFFER_SIZE,
            STATUS_INVALID_DEVICE_REQUEST, STATUS_SUCCESS, STATUS_UNSUCCESSFUL,
        },
    },
};

const DEVICE_NAME: &str = "\\Device\\SysMon";
const SYM_LINK_NAME: &str = "\\??\\SysMon";

const MAX_ITEM_COUNT: usize = 256;

static mut G_EVENTS: Option<Vec<ItemInfo>> = None;
static mut G_MUTEX: FastMutex = FastMutex::new();
static mut G_COOKIE: LARGE_INTEGER = LARGE_INTEGER::new();

#[no_mangle]
pub unsafe extern "system" fn DriverEntry(
    driver: &mut DRIVER_OBJECT,
    _path: *const UNICODE_STRING,
) -> NTSTATUS {
    KernelLogger::init(LevelFilter::Trace).expect("Failed to initialize logger");
    log::info!("START");

    G_MUTEX.Init();

    let mut events = Vec::new();

    log::trace!("Before reverse");
    if let Err(e) = events.try_reserve_exact(MAX_ITEM_COUNT) {
        log::error!(
            "fail to reserve a {} bytes of memory. Err: {:?}",
            ::core::mem::size_of::<ItemInfo>() * MAX_ITEM_COUNT,
            e
        );
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    G_EVENTS = Some(events);

    driver.DriverUnload = Some(DriverUnload);

    driver.MajorFunction[IRP_MJ::CREATE as usize] = Some(DispatchCreateClose);
    driver.MajorFunction[IRP_MJ::CLOSE as usize] = Some(DispatchCreateClose);
    driver.MajorFunction[IRP_MJ::DEVICE_CONTROL as usize] = Some(DispatchDeviceControl);
    driver.MajorFunction[IRP_MJ::READ as usize] = Some(DispatchRead);
    driver.MajorFunction[IRP_MJ::WRITE as usize] = Some(DispatchWrite);

    #[allow(unused_assignments)]
    let mut status = STATUS_SUCCESS;

    let hello_world = UNICODE_STRING::create("Hello World!");
    log::info!("{}", hello_world.as_rust_string().unwrap_or_default());

    let dev_name = UNICODE_STRING::from(DEVICE_NAME);
    let sym_link = UNICODE_STRING::from(SYM_LINK_NAME);

    let mut cleaner = Cleaner::new();
    let mut device_object: PDEVICE_OBJECT = null_mut();

    loop {
        //--------------------DEVICE-----------------------
        status = IoCreateDevice(
            driver,
            0,
            dev_name.as_ptr(),
            DEVICE_TYPE::FILE_DEVICE_UNKNOWN,
            0,
            FALSE,
            &mut device_object,
        );

        if NT_SUCCESS!(status) {
            cleaner.init_device(device_object);
        } else {
            log::error!("failed to create device 0x{:08x}", status);
            break;
        }
        log::info!("Device create!");

        (*device_object).Flags |= DO_DIRECT_IO as u32;

        //--------------------SYMLINK-----------------------
        status = IoCreateSymbolicLink(&sym_link.as_ntdef_unicode(), &dev_name.as_ntdef_unicode());

        if NT_SUCCESS!(status) {
            cleaner.init_symlink(&sym_link);
        } else {
            log::error!("failed to create sym_link 0x{:08x}", status);
            break;
        }
        log::info!("Device symlink!");

        //--------------------PROCESS NOTIFY-----------------------
        status = PsSetCreateProcessNotifyRoutineEx(OnProcessNotify, FALSE);

        if NT_SUCCESS!(status) {
            cleaner.init_process_create_callback(OnProcessNotify);
        } else {
            log::error!("failed to create process nofity rountine 0x{:08x}", status);
            break;
        }
        log::info!("ProcessNotify created");

        //--------------------THREAD NOTIFY-----------------------
        status = PsSetCreateThreadNotifyRoutine(OnThreadNotify);

        if NT_SUCCESS!(status) {
            cleaner.init_thread_create_callback(OnThreadNotify);
        } else {
            log::error!("failed to create thread nofity rountine 0x{:08x}", status);
            break;
        }
        log::info!("ThreadNotify created");

        //--------------------IMAGE NOTIFY-----------------------
        status = PsSetLoadImageNotifyRoutine(OnImageLoadNotify);

        if NT_SUCCESS!(status) {
            cleaner.init_image_load_callback(OnImageLoadNotify);
        } else {
            log::error!("failed to create image load routine 0x{:08x}", status);
            break;
        }
        log::info!("ImageNotify created");

        //--------------------REGISTRY NOTIFY-----------------------
        let altitude = UNICODE_STRING::create("7657.124");
        status = CmRegisterCallbackEx(
            OnRegistryNotify as PVOID,
            &altitude,
            driver,
            null_mut(),
            &G_COOKIE,
            null_mut(),
        );

        if NT_SUCCESS!(status) {
            cleaner.init_registry_callback(G_COOKIE);
        } else {
            log::error!("failed to create registry routine 0x{:08x}", status);
            break;
        }
        log::info!("RegistryNotify created");

        break;
    }

    if NT_SUCCESS!(status) {
        log::info!("SUCCESS");
    } else {
        cleaner.clean();
    }

    status
}

extern "system" fn DriverUnload(driver: &mut DRIVER_OBJECT) {
    log::info!("rust_unload");
    unsafe {
        IoDeleteDevice(driver.DeviceObject);

        let sym_link = UNICODE_STRING::create(SYM_LINK_NAME);
        IoDeleteSymbolicLink(&sym_link.as_ntdef_unicode());

        PsSetCreateProcessNotifyRoutineEx(OnProcessNotify, TRUE);

        PsRemoveCreateThreadNotifyRoutine(OnThreadNotify);

        PsRemoveLoadImageNotifyRoutine(OnImageLoadNotify);

        CmUnRegisterCallback(G_COOKIE);
    }
}

extern "system" fn DispatchCreateClose(_driver: &mut DEVICE_OBJECT, irp: &mut IRP) -> NTSTATUS {
    complete_irp_success(irp)
}

extern "system" fn DispatchDeviceControl(_driver: &mut DEVICE_OBJECT, irp: &mut IRP) -> NTSTATUS {
    unsafe {
        let stack = IoGetCurrentIrpStackLocation(irp);
        let device_io = (*stack).Parameters.DeviceIoControl();

        match device_io.IoControlCode {
            ioctl_code::IOCTL_REQUEST => log::info!("device control success"),
            _ => {
                return complete_irp_with_status(irp, STATUS_INVALID_DEVICE_REQUEST);
            },
        }
    }

    complete_irp_success(irp)
}

extern "system" fn DispatchRead(_driver: &mut DEVICE_OBJECT, irp: &mut IRP) -> NTSTATUS {
    log::info!("DispatchRead begin");

    unsafe {
        let stack = IoGetCurrentIrpStackLocation(irp);
        let parameters_read = (*stack).ParametersRead();
        let len = parameters_read.Length;

        log::info!("read len: {}", len);

        if len == 0 {
            log::info!("len is zero");
            return complete_irp_with_status(irp, STATUS_INVALID_BUFFER_SIZE);
        }

        let buffer = MmGetSystemAddressForMdlSafe(
            irp.MdlAddress as *mut MDL,
            16, /*NormalPagePriority*/
        );
        if buffer.is_null() {
            log::info!("buffer is null");
            return complete_irp_with_status(irp, STATUS_INSUFFICIENT_RESOURCES);
        }

        let buffer = buffer as *mut u8;
        let copied_bytes = copy_events_to_ptr(buffer, len as usize);
        if copied_bytes == 0 {
            return complete_irp_with_status(irp, STATUS_INSUFFICIENT_RESOURCES);
        }

        log::info!("DispatchRead success, copied bytes: {copied_bytes}");

        complete_irp(irp, STATUS_SUCCESS, copied_bytes)
    }
}

#[allow(non_camel_case_types)]
pub type _IO_STACK_LOCATION_WRITE = _IO_STACK_LOCATION_READ;
pub fn ParametersWrite(stack_loc: &mut IO_STACK_LOCATION) -> &mut _IO_STACK_LOCATION_WRITE {
    stack_loc.ParametersRead()
}

extern "system" fn DispatchWrite(_driver: &mut DEVICE_OBJECT, irp: &mut IRP) -> NTSTATUS {
    log::info!("DispatchWrite begin");

    unsafe {
        let stack = IoGetCurrentIrpStackLocation(irp);
        let stack = &mut *stack;
        let parameters_write = ParametersWrite(stack);

        let len = parameters_write.Length;

        complete_irp(irp, STATUS_SUCCESS, len as usize)
    }
}
fn complete_irp(irp: &mut IRP, status: NTSTATUS, info: usize) -> NTSTATUS {
    unsafe {
        let s = irp.IoStatus.__bindgen_anon_1.Status_mut();
        *s = status;
        irp.IoStatus.Information = info;
        IoCompleteRequest(irp, 0);
    }

    status
}

extern "system" fn OnProcessNotify(
    _process: PEPROCESS,
    process_id: HANDLE,
    create_info: PPS_CREATE_NOTIFY_INFO,
) {
    unsafe {
        //kernel_print!("process create");
        let item = if !create_info.is_null() {
            let create_info: &PS_CREATE_NOTIFY_INFO = &*create_info;
            let create_info: &PS_CREATE_NOTIFY_INFO = &*create_info;

            let image_file_name = &*create_info.ImageFileName;
            ProcessCreate {
                pid: HandleToU32!(process_id),
                parent_pid: HandleToU32!(create_info.ParentProcessId),
                command_line: ItemInfo::string_to_buffer(
                    image_file_name.as_rust_string().unwrap_or_default(),
                ),
            }
        } else {
            ProcessExit {
                pid: HandleToU32!(process_id),
            }
        };

        push_item_thread_safe(item);
    }
}

extern "system" fn OnThreadNotify(process_id: HANDLE, thread_id: HANDLE, create: BOOLEAN) {
    unsafe {
        //kernel_print!("thread create");

        let item = if create == TRUE {
            ThreadCreate {
                pid: HandleToU32!(process_id),
                tid: HandleToU32!(thread_id),
            }
        } else {
            ThreadExit {
                pid: HandleToU32!(process_id),
                tid: HandleToU32!(thread_id),
            }
        };

        push_item_thread_safe(item);
    }
}

extern "system" fn OnImageLoadNotify(
    full_image_name: PUNICODE_STRING,
    process_id: HANDLE,
    image_info: PIMAGE_INFO,
) {
    if process_id.is_null() {
        // system image, ignore
        return;
    }

    unsafe {
        //kernel_print!("image load");

        let image_name = if full_image_name.is_null() {
            "(unknown)".to_string()
        } else {
            (*full_image_name)
                .as_rust_string()
                .unwrap_or("(unknown)".to_string())
        };

        let image_info = &*image_info;
        let item = ImageLoad {
            pid: HandleToU32!(process_id),
            load_address: image_info.ImageBase as isize,
            image_size: image_info.ImageSize,
            image_file_name: ItemInfo::string_to_buffer(image_name),
        };

        push_item_thread_safe(item);
    }
}

extern "system" fn OnRegistryNotify(_context: PVOID, arg1: PVOID, arg2: PVOID) -> NTSTATUS {
    let reg_notify = HandleToU32!(arg1);
    if reg_notify == REG_NT_POST_SET_VALUE_KEY {
        //kernel_print!("RegNtPostSetValueKey");
        unsafe {
            let op_info = &*(arg2 as PREG_POST_OPERATION_INFORMATION);
            if !NT_SUCCESS!(op_info.Status) {
                return STATUS_SUCCESS;
            }

            let mut name: PUNICODE_STRING = null_mut();
            let status =
                CmCallbackGetKeyObjectIDEx(&G_COOKIE, op_info.Object, null_mut(), &mut name, 0);
            if !NT_SUCCESS!(status) {
                return STATUS_SUCCESS;
            }

            if name.is_null() {
                //something wrong
                return STATUS_UNSUCCESSFUL;
            }

            loop {
                let key_name = if let Some(key_name) = (*name).as_rust_string() {
                    key_name
                } else {
                    //log::info!("Something wrong. Can't convert \"key_name\" to rust string");
                    break;
                };
                let registry_machine = "\\REGISTRY\\MACHINE";

                // filter out none-HKLM writes
                if key_name.contains(registry_machine) {
                    if op_info.PreInformation.is_null() {
                        //something wrong
                        break;
                    }

                    let pre_info = &*(op_info.PreInformation as PREG_SET_VALUE_KEY_INFORMATION);
                    let _value_name = if let Some(value_name) =
                        (*pre_info.ValueName).as_rust_string()
                    {
                        value_name
                    } else {
                        log::error!("Something wrong. Can't convert \"value_name\" to rust string");
                        break;
                    };

                    // let v = if pre_info.DataSize > 0 {
                    //     Vec::from_raw_parts(
                    //         pre_info.Data as *mut u8,
                    //         pre_info.DataSize as usize,
                    //         pre_info.DataSize as usize,
                    //     )
                    // } else {
                    //     Vec::new()
                    // };

                    let item = RegistrySetValue {
                        pid: HandleToU32!(PsGetCurrentProcessId()),
                        tid: HandleToU32!(PsGetCurrentThreadId()),
                        key_name: ItemInfo::string_to_buffer(key_name),
                        // value_name,
                        data_type: pre_info.DataType,
                        // data: v.clone(),
                    };

                    //forget(v);
                    push_item_thread_safe(item);
                }
                break;
            }
            CmCallbackReleaseKeyObjectIDEx(name);
        }
    }

    STATUS_SUCCESS
}

unsafe fn push_item_thread_safe(item: ItemInfo) {
    let _locker = AutoLock::new(&mut G_MUTEX);
    if let Some(events) = &mut G_EVENTS {
        if events.len() >= MAX_ITEM_COUNT {
            events.remove(0);
        }
        events.push(item);
    }
}

unsafe fn copy_events_to_ptr(dst_ptr: *mut u8, buffer_size: usize) -> usize {
    let _locker = AutoLock::new(&mut G_MUTEX);
    if let Some(events) = &mut G_EVENTS {
        let events_byte_size = events.len() * size_of::<ItemInfo>();
        if events_byte_size > buffer_size {
            log::info!(
                "Buff to small: size: {}, necessary size: {}",
                buffer_size,
                events_byte_size
            );
            return 0;
        }

        let events_ptr = events.as_mut_ptr() as *mut u8;
        ptr::copy_nonoverlapping(events_ptr, dst_ptr, events_byte_size);

        events.clear();
        return events_byte_size;
    }
    0
}

fn complete_irp_with_status(irp: &mut IRP, status: NTSTATUS) -> NTSTATUS {
    complete_irp(irp, status, 0)
}

fn complete_irp_success(irp: &mut IRP) -> NTSTATUS {
    complete_irp_with_status(irp, STATUS_SUCCESS)
}
