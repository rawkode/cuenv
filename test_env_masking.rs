use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::io::Write;

fn main() {
    // This test demonstrates the issue with env command output
    
    // Simulate what happens when running `cuenv run env`
    // The env command outputs lines like: SECRET_KEY=mysecretvalue
    
    let mut secrets = HashSet::new();
    secrets.insert("mysecretvalue".to_string());
    let secrets = Arc::new(Mutex::new(secrets));
    
    // Test 1: Direct secret output (this works)
    {
        let mut output = Vec::new();
        let mut filter = cuenv::output_filter::OutputFilter::new(&mut output, secrets.clone());
        
        write!(filter, "The secret is: mysecretvalue\n").unwrap();
        
        let result = String::from_utf8(output).unwrap();
        println!("Test 1 (direct output): {}", result);
        assert!(result.contains("***********"));
        assert!(!result.contains("mysecretvalue"));
    }
    
    // Test 2: Environment variable format (this is the issue)
    {
        let mut output = Vec::new();
        let mut filter = cuenv::output_filter::OutputFilter::new(&mut output, secrets.clone());
        
        // This is what the env command outputs
        write!(filter, "SECRET_KEY=mysecretvalue\n").unwrap();
        write!(filter, "OTHER_VAR=public-value\n").unwrap();
        
        let result = String::from_utf8(output).unwrap();
        println!("Test 2 (env format): {}", result);
        
        // This test shows that the secret IS being masked even in env format
        assert!(result.contains("SECRET_KEY=***********"));
        assert!(!result.contains("mysecretvalue"));
    }
}