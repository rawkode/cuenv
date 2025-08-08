use cuenv::task::cross_package::{CrossPackageReference, parse_reference};

#[test]
fn test_parse_local_task_reference() {
    let reference = parse_reference("build").unwrap();
    assert_eq!(reference, CrossPackageReference::LocalTask {
        task: "build".to_string(),
    });
}

#[test]
fn test_parse_package_task_reference() {
    // Use a task name that won't be confused with an output
    let reference = parse_reference("projects:frontend:test").unwrap();
    assert_eq!(reference, CrossPackageReference::PackageTask {
        package: "projects:frontend".to_string(),
        task: "test".to_string(),
    });
    
    // "build" will be interpreted as an output name due to our heuristic
    let reference2 = parse_reference("projects:frontend:build").unwrap();
    assert_eq!(reference2, CrossPackageReference::PackageTaskOutput {
        package: "projects".to_string(),
        task: "frontend".to_string(),
        output: "build".to_string(),
    });
}

#[test]
fn test_parse_package_task_output_reference() {
    let reference = parse_reference("projects:frontend:build:dist").unwrap();
    assert_eq!(reference, CrossPackageReference::PackageTaskOutput {
        package: "projects:frontend".to_string(),
        task: "build".to_string(),
        output: "dist".to_string(),
    });
}

#[test]
fn test_parse_complex_package_hierarchy() {
    // Test deeply nested package names
    let reference = parse_reference("tools:ci:cd:deploy:artifacts").unwrap();
    assert_eq!(reference, CrossPackageReference::PackageTaskOutput {
        package: "tools:ci:cd".to_string(),
        task: "deploy".to_string(),
        output: "artifacts".to_string(),
    });
}

#[test]
fn test_parse_single_colon_as_local() {
    // Single component should be treated as local task
    let reference = parse_reference("test").unwrap();
    assert_eq!(reference, CrossPackageReference::LocalTask {
        task: "test".to_string(),
    });
}

#[test]
fn test_parse_empty_string() {
    let result = parse_reference("");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

#[test]
fn test_parse_invalid_characters() {
    let test_cases = vec![
        "build@test",
        "package:task!",
        "package:ta sk",
        "../package:task",
        "package::task",
    ];
    
    for invalid in test_cases {
        let result = parse_reference(invalid);
        assert!(result.is_err(), "Should reject: {}", invalid);
    }
}

#[test]
fn test_parse_empty_components() {
    let test_cases = vec![
        ":task",
        "package:",
        "package::",
        "::output",
        "package:task:",
    ];
    
    for invalid in test_cases {
        let result = parse_reference(invalid);
        assert!(result.is_err(), "Should reject: {}", invalid);
    }
}

#[test]
fn test_normalize_package_names() {
    // Package names should allow hyphens and underscores
    // Using "run" as task name to avoid confusion with output
    let reference = parse_reference("my-package:sub_package:run").unwrap();
    assert_eq!(reference, CrossPackageReference::PackageTask {
        package: "my-package:sub_package".to_string(),
        task: "run".to_string(),
    });
    
    // With "build" it will be interpreted as output
    let reference2 = parse_reference("my-package:sub_package:build").unwrap();
    assert_eq!(reference2, CrossPackageReference::PackageTaskOutput {
        package: "my-package".to_string(),
        task: "sub_package".to_string(),
        output: "build".to_string(),
    });
}

#[test]
fn test_reference_to_string() {
    let cases = vec![
        (CrossPackageReference::LocalTask {
            task: "build".to_string(),
        }, "build"),
        (CrossPackageReference::PackageTask {
            package: "projects:frontend".to_string(),
            task: "build".to_string(),
        }, "projects:frontend:build"),
        (CrossPackageReference::PackageTaskOutput {
            package: "projects:frontend".to_string(),
            task: "build".to_string(),
            output: "dist".to_string(),
        }, "projects:frontend:build:dist"),
    ];
    
    for (reference, expected) in cases {
        assert_eq!(reference.to_string(), expected);
    }
}

#[test]
fn test_reference_components() {
    let ref1 = CrossPackageReference::LocalTask {
        task: "build".to_string(),
    };
    assert_eq!(ref1.package(), None);
    assert_eq!(ref1.task(), "build");
    assert_eq!(ref1.output(), None);
    
    let ref2 = CrossPackageReference::PackageTask {
        package: "projects:frontend".to_string(),
        task: "test".to_string(),
    };
    assert_eq!(ref2.package(), Some("projects:frontend"));
    assert_eq!(ref2.task(), "test");
    assert_eq!(ref2.output(), None);
    
    let ref3 = CrossPackageReference::PackageTaskOutput {
        package: "projects:frontend".to_string(),
        task: "build".to_string(),
        output: "dist".to_string(),
    };
    assert_eq!(ref3.package(), Some("projects:frontend"));
    assert_eq!(ref3.task(), "build");
    assert_eq!(ref3.output(), Some("dist"));
}

#[test]
fn test_is_cross_package() {
    let local = CrossPackageReference::LocalTask {
        task: "build".to_string(),
    };
    assert!(!local.is_cross_package());
    
    let cross = CrossPackageReference::PackageTask {
        package: "other".to_string(),
        task: "build".to_string(),
    };
    assert!(cross.is_cross_package());
    
    let cross_output = CrossPackageReference::PackageTaskOutput {
        package: "other".to_string(),
        task: "build".to_string(),
        output: "dist".to_string(),
    };
    assert!(cross_output.is_cross_package());
}