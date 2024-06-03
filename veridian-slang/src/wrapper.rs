include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::ffi::{CStr, CString};
    use std::fs::File;
    use std::io::Write;
    use tempdir::TempDir;

    #[test]
    #[serial]
    fn test_path() {
        let dir = TempDir::new("slang_wrapper_tests").unwrap();
        let file_path = dir.path().join("test.sv");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(b"module test; logic [1:0] abc; assign abc[2] = 1'b1; endmodule")
            .unwrap();
        f.sync_all().unwrap();

        let path = CString::new(file_path.to_str().unwrap()).unwrap();
        let report_raw = unsafe { compile_path(path.as_ptr()) };
        let report: &CStr = unsafe { CStr::from_ptr(report_raw) };
        let expected =
            ":1:42: warning: cannot refer to element 2 of \'logic[1:0]\' [-Windex-oob]\n";
        let mut result = report.to_str().unwrap().to_owned();
        let offset = result.find(':').unwrap_or_else(|| result.len());
        result.replace_range(..offset, "");
        assert_eq!(result, expected);
        unsafe {
            delete_report(report_raw);
        }
    }

    #[test]
    #[serial]
    fn test_paths() {
        let dir = TempDir::new("slang_wrapper_tests").unwrap();
        let file_path_1 = dir.path().join("test1.sv");
        let file_path_1_c = CString::new(file_path_1.to_str().unwrap()).unwrap();
        let mut f = File::create(&file_path_1).unwrap();
        f.write_all(b"module test1; logic [1:0] abc; assign abc[2] = 1'b1; endmodule")
            .unwrap();
        f.sync_all().unwrap();

        let file_path_2 = dir.path().join("test2.sv");
        let file_path_2_c = CString::new(file_path_2.to_str().unwrap()).unwrap();
        let mut f = File::create(&file_path_2).unwrap();
        f.write_all(b"module test2; logic [1:0] abc; assign abc[2] = 1'b1; endmodule")
            .unwrap();
        f.sync_all().unwrap();

        let file_path_3 = dir.path().join("test3.sv");
        let file_path_3_c = CString::new(file_path_3.to_str().unwrap()).unwrap();
        let mut f = File::create(&file_path_3).unwrap();
        f.write_all(b"module test3; logic [1:0] abc; assign abc[2] = 1'b1; endmodule")
            .unwrap();
        f.sync_all().unwrap();

        let mut paths: Vec<*const i8> = Vec::new();
        paths.push(file_path_1_c.as_ptr());
        paths.push(file_path_2_c.as_ptr());
        paths.push(file_path_3_c.as_ptr());
        let report_raw = unsafe { compile_paths(paths.as_mut_ptr(), 3) };
        let report: &CStr = unsafe { CStr::from_ptr(report_raw) };
        let mut expected =
            ":1:43: warning: cannot refer to element 2 of \'logic[1:0]\' [-Windex-oob]\n".repeat(3);
        expected.pop();
        let result_raw = report.to_str().unwrap().to_owned();
        let result_iter = result_raw.lines();
        let mut result: String = String::new();
        result_iter.for_each(|x| {
            let mut y: String = x.to_owned();
            y.replace_range(..x.find(":").unwrap(), "\n");
            result.push_str(&y);
        });
        // get rid of unnecessary newlines
        result = result.trim_start().to_owned();
        assert_eq!(result, expected);
        unsafe {
            delete_report(report_raw);
        }
    }

    #[test]
    #[serial]
    fn test_compilation_multi() {
        let mut names: Vec<*const i8> = Vec::new();
        let mut texts: Vec<*const i8> = Vec::new();
        let name1 = CString::new("test1.sv").unwrap();
        let name2 = CString::new("test2.sv").unwrap();
        let name3 = CString::new("test3.sv").unwrap();
        names.push(name1.as_ptr());
        names.push(name2.as_ptr());
        names.push(name3.as_ptr());
        let text1 =
            CString::new("module test1; logic [1:0] abc; assign abc[2] = 1'b1; endmodule").unwrap();
        let text2 =
            CString::new("module test2; logic [1:0] abc; assign abc[2] = 1'b1; endmodule").unwrap();
        let text3 =
            CString::new("module test3; logic [1:0] abc; assign abc[2] = 1'b1; endmodule").unwrap();
        texts.push(text1.as_ptr());
        texts.push(text2.as_ptr());
        texts.push(text3.as_ptr());

        let report_raw = unsafe { compile_sources(names.as_mut_ptr(), texts.as_mut_ptr(), 3) };
        let report: &CStr = unsafe { CStr::from_ptr(report_raw) };
        let expected = "test1.sv:1:43: warning: cannot refer to element 2 of \'logic[1:0]\' [-Windex-oob]\ntest2.sv:1:43: warning: cannot refer to element 2 of \'logic[1:0]\' [-Windex-oob]\ntest3.sv:1:43: warning: cannot refer to element 2 of \'logic[1:0]\' [-Windex-oob]\n";
        let result = report.to_str().unwrap();
        assert_eq!(result, expected);
        unsafe {
            delete_report(report_raw);
        }
    }
}
