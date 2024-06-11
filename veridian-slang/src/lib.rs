#![allow(dead_code)]
use std::error;
use std::ffi::{CStr, CString};
use std::path::PathBuf;

mod wrapper;
use wrapper::*;

pub fn slang_compile(paths: Vec<PathBuf>) -> Result<String, Box<dyn error::Error>> {
    if paths.is_empty() {
        return Ok(String::new());
    }

    // convert pathbufs to strings
    let mut paths_str: Vec<String> = Vec::new();
    for path in paths {
        paths_str.push(path.to_str().unwrap().to_owned());
    }
    // convert strings to cstrings
    let mut paths_c: Vec<CString> = Vec::new();
    for path in paths_str {
        paths_c.push(CString::new(path)?);
    }
    // convert cstrings to char* pointers
    let mut paths_ptr: Vec<*const std::ffi::c_char> = paths_c.iter().map(|x| x.as_ptr()).collect();

    // compile with slang, and convert report from char* to string
    let report_raw = unsafe { compile_paths(paths_ptr.as_mut_ptr(), paths_ptr.len() as u32) };
    let report: &CStr = unsafe { CStr::from_ptr(report_raw) };
    let result = report.to_str()?.to_owned();
    unsafe {
        delete_report(report_raw);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs::File;
    use std::io::Write;
    use tempdir::TempDir;

    #[test]
    #[serial]
    fn test_paths_wrapper() {
        let dir = TempDir::new("slang_wrapper_tests").unwrap();
        let file_path_1 = dir.path().join("test1.sv");
        let mut f = File::create(&file_path_1).unwrap();
        f.write_all(b"module test1; logic [1:0] abc; assign abc[2] = 1'b1; endmodule")
            .unwrap();
        f.sync_all().unwrap();

        let file_path_2 = dir.path().join("test2.sv");
        let mut f = File::create(&file_path_2).unwrap();
        f.write_all(b"module test2; logic [1:0] abc; assign abc[2] = 1'b1; endmodule")
            .unwrap();
        f.sync_all().unwrap();

        let file_path_3 = dir.path().join("test3.sv");
        let mut f = File::create(&file_path_3).unwrap();
        f.write_all(b"module test3; logic [1:0] abc; assign abc[2] = 1'b1; endmodule")
            .unwrap();
        f.sync_all().unwrap();

        let mut paths: Vec<PathBuf> = Vec::new();
        paths.push(file_path_1);
        paths.push(file_path_2);
        paths.push(file_path_3);

        let report: String = slang_compile(paths).unwrap();
        let mut expected =
            ":1:43: warning: cannot refer to element 2 of \'logic[1:0]\' [-Windex-oob]\n".repeat(3);
        expected.pop();
        let result_iter = report.lines();
        let mut result: String = String::new();
        result_iter.for_each(|x| {
            let mut y: String = x.to_owned();
            y.replace_range(..x.find(':').unwrap(), "\n");
            result.push_str(&y);
        });
        // get rid of unnecessary newlines
        result = result.trim_start().to_owned();
        assert_eq!(result, expected);
    }
}
