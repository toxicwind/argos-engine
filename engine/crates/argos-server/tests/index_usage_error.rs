use std::process::Command;

#[test]
fn argos_index_refuses_a_directory_before_touching_disk() {
    let file_target = tempfile::tempdir().expect("tempdir for --file");
    let scratch = tempfile::tempdir().expect("tempdir for --data-dir");
    let data_dir = scratch.path().join("data");

    let out = Command::new(env!("CARGO_BIN_EXE_argos"))
        .args(["index", "--index", "probe", "--file"])
        .arg(file_target.path())
        .arg("--data-dir")
        .arg(&data_dir)
        .output()
        .expect("run the argos binary");

    assert_eq!(
        out.status.code(),
        Some(2),
        "expected exit 2 for a usage error, got {:?}\nstderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr),
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.is_empty(), "stdout must be empty, got:\n{stdout}");

    let stderr = String::from_utf8_lossy(&out.stderr);
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 2, "stderr must be two lines, got:\n{stderr}");
    assert!(lines[0].starts_with("error: "), "{stderr}");
    assert!(lines[0].contains("is a directory"), "{stderr}");
    assert!(lines[0].contains("argos autoindex"), "{stderr}");
    assert!(lines[1].contains("argos index --help"), "{stderr}");

    assert!(
        !data_dir.exists(),
        "a refused --file must not create the data dir: {}",
        data_dir.display()
    );
}
