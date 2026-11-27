use std::ffi::{c_char, c_int, c_void, CStr};

type FnAcerOpen = unsafe extern "C" fn() -> *mut c_void;
type FnAcerClose = unsafe extern "C" fn(*mut c_void);
type FnAcerVersion = unsafe extern "C" fn() -> *const c_char;
type FnAcerIntMetric = unsafe extern "C" fn(*mut c_void, *mut c_int) -> c_int;
type FnAcerStrMetric = unsafe extern "C" fn(*mut c_void, *mut c_char, libc::c_uint) -> c_int;

pub struct AcerControlLib {
    lib_handle: *mut c_void,
    open_fn: FnAcerOpen,
    close_fn: FnAcerClose,
    version_fn: Option<FnAcerVersion>,
    cpu_fan_rpm_fn: FnAcerIntMetric,
    gpu_fan_rpm_fn: FnAcerIntMetric,
    cpu_temp_fn: FnAcerIntMetric,
    gpu_temp_fn: FnAcerIntMetric,
    driver_interface_fn: Option<FnAcerStrMetric>,
    driver_version_fn: Option<FnAcerStrMetric>,
}

unsafe impl Send for AcerControlLib {}
unsafe impl Sync for AcerControlLib {}

impl AcerControlLib {
    pub fn load() -> Result<Self, String> {
        const CANDIDATES: [&[u8]; 3] = [
            b"libacercontrol.so.0\0",
            b"/usr/lib64/libacercontrol.so.0\0",
            b"/usr/local/lib64/libacercontrol.so\0",
        ];

        let mut lib_handle: *mut c_void = std::ptr::null_mut();
        for soname in CANDIDATES {
            lib_handle = unsafe { libc::dlopen(soname.as_ptr() as *const c_char, libc::RTLD_LAZY) };
            if !lib_handle.is_null() {
                break;
            }
        }

        if lib_handle.is_null() {
            return Err("libacercontrol.so.0 could not be loaded".into());
        }

        macro_rules! sym {
            ($name:expr) => {{
                let ptr = unsafe { libc::dlsym(lib_handle, $name.as_ptr() as *const c_char) };
                if ptr.is_null() {
                    unsafe { libc::dlclose(lib_handle) };
                    return Err(format!(
                        "Symbol {} missing in libacercontrol",
                        String::from_utf8_lossy($name).trim_end_matches('\0')
                    ));
                }
                unsafe { std::mem::transmute(ptr) }
            }};
        }

        let open_fn: FnAcerOpen = sym!(b"acer_open\0");
        let close_fn: FnAcerClose = sym!(b"acer_close\0");
        let cpu_fan_rpm_fn: FnAcerIntMetric = sym!(b"acer_cpu_fan_rpm\0");
        let gpu_fan_rpm_fn: FnAcerIntMetric = sym!(b"acer_gpu_fan_rpm\0");
        let cpu_temp_fn: FnAcerIntMetric = sym!(b"acer_cpu_temperature\0");
        let gpu_temp_fn: FnAcerIntMetric = sym!(b"acer_gpu_temperature\0");

        let ver_ptr = unsafe { libc::dlsym(lib_handle, b"acercontrol_version\0".as_ptr() as *const c_char) };
        let version_fn: Option<FnAcerVersion> = if ver_ptr.is_null() {
            None
        } else {
            Some(unsafe { std::mem::transmute(ver_ptr) })
        };

        let if_ptr = unsafe { libc::dlsym(lib_handle, b"acer_driver_interface\0".as_ptr() as *const c_char) };
        let driver_interface_fn: Option<FnAcerStrMetric> = if if_ptr.is_null() {
            None
        } else {
            Some(unsafe { std::mem::transmute(if_ptr) })
        };

        let drv_ver_ptr = unsafe { libc::dlsym(lib_handle, b"acer_driver_version\0".as_ptr() as *const c_char) };
        let driver_version_fn: Option<FnAcerStrMetric> = if drv_ver_ptr.is_null() {
            None
        } else {
            Some(unsafe { std::mem::transmute(drv_ver_ptr) })
        };

        Ok(Self {
            lib_handle,
            open_fn,
            close_fn,
            version_fn,
            cpu_fan_rpm_fn,
            gpu_fan_rpm_fn,
            cpu_temp_fn,
            gpu_temp_fn,
            driver_interface_fn,
            driver_version_fn,
        })
    }

    #[allow(dead_code)]
    pub fn version(&self) -> Option<String> {
        let f = self.version_fn?;
        let ptr = unsafe { f() };
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned())
        }
    }

    pub fn open_context(&self) -> Result<AcerContextGuard<'_>, String> {
        let ptr = unsafe { (self.open_fn)() };
        if ptr.is_null() {
            Err(Self::diagnose_open_failure())
        } else {
            Ok(AcerContextGuard { lib: self, ptr })
        }
    }

    fn diagnose_open_failure() -> String {
        let path = std::path::Path::new("/dev/acer_io");
        if !path.exists() {
            "Hardware interface /dev/acer_io is missing (ensure acer_io kernel module is loaded)".into()
        } else {
            match std::fs::OpenOptions::new().read(true).write(true).open(path) {
                Ok(_) => "Failed to open Acer hardware context".into(),
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                    "Permission denied for /dev/acer_io (user must belong to 'acercontrol' group)".into()
                }
                Err(err) => format!("Unable to open /dev/acer_io: {err}"),
            }
        }
    }
}

impl Drop for AcerControlLib {
    fn drop(&mut self) {
        if !self.lib_handle.is_null() {
            unsafe { libc::dlclose(self.lib_handle) };
            self.lib_handle = std::ptr::null_mut();
        }
    }
}

pub struct AcerContextGuard<'a> {
    lib: &'a AcerControlLib,
    ptr: *mut c_void,
}

impl AcerContextGuard<'_> {
    fn read_metric(&self, f: FnAcerIntMetric, label: &str) -> Result<i32, String> {
        let mut value: c_int = 0;
        let ret = unsafe { f(self.ptr, &mut value) };
        if ret < 0 {
            Err(format!("Failed to read {label}"))
        } else {
            Ok(value)
        }
    }

    pub fn cpu_fan_rpm(&self) -> Result<i32, String> {
        self.read_metric(self.lib.cpu_fan_rpm_fn, "CPU fan RPM")
    }

    pub fn gpu_fan_rpm(&self) -> Result<i32, String> {
        self.read_metric(self.lib.gpu_fan_rpm_fn, "GPU fan RPM")
    }

    pub fn cpu_temperature(&self) -> Result<i32, String> {
        self.read_metric(self.lib.cpu_temp_fn, "CPU temperature")
    }

    pub fn gpu_temperature(&self) -> Result<i32, String> {
        self.read_metric(self.lib.gpu_temp_fn, "GPU temperature")
    }

    pub fn driver_interface(&self) -> Option<String> {
        let f = self.lib.driver_interface_fn?;
        let mut buf = [0u8; 64];
        let ret = unsafe { f(self.ptr, buf.as_mut_ptr() as *mut c_char, buf.len() as libc::c_uint) };
        if ret < 0 {
            return None;
        }
        let s = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) }
            .to_string_lossy()
            .trim()
            .to_string();
        if s.is_empty() { None } else { Some(s) }
    }

    pub fn driver_version(&self) -> Option<String> {
        let f = self.lib.driver_version_fn?;
        let mut buf = [0u8; 64];
        let ret = unsafe { f(self.ptr, buf.as_mut_ptr() as *mut c_char, buf.len() as libc::c_uint) };
        if ret < 0 {
            return None;
        }
        let s = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) }
            .to_string_lossy()
            .trim()
            .to_string();
        if s.is_empty() { None } else { Some(s) }
    }
}

impl Drop for AcerContextGuard<'_> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { (self.lib.close_fn)(self.ptr) };
            self.ptr = std::ptr::null_mut();
        }
    }
}
