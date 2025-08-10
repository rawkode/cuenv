use cuenv_config::Config;
use cuenv_core::{Result, ENV_CUE_FILENAME};
use std::sync::Arc;

pub async fn execute(config: Arc<Config>, force: bool) -> Result<()> {
    let env_file = config.working_dir.join(ENV_CUE_FILENAME);

    if env_file.exists() && !force {
        eprintln!("Error: {ENV_CUE_FILENAME} already exists. Use --force to overwrite.");
        std::process::exit(1);
    }

    let template = r#"package main

import "cuenv.io/env"

// Define your environment configuration
environment: env.#Environment & {
    // Development environment
    dev: {
        variables: {
            NODE_ENV: {
                value: "development"
                description: "Node environment"
            }
            DEBUG: {
                value: "true"
                description: "Enable debug mode"
            }
        }
    }
    
    // Production environment
    production: {
        variables: {
            NODE_ENV: {
                value: "production"
                description: "Node environment"
            }
            DEBUG: {
                value: "false"
                description: "Disable debug mode"
            }
        }
    }
}

// Define tasks
tasks: env.#Tasks & {
    dev: {
        description: "Start development server"
        command: ["npm", "run", "dev"]
    }
    
    build: {
        description: "Build for production"
        command: ["npm", "run", "build"]
    }
    
    test: {
        description: "Run tests"
        command: ["npm", "test"]
    }
}
"#;

    std::fs::write(&env_file, template)
        .map_err(|e| cuenv_core::Error::file_system(&env_file, "write", e))?;

    println!("✓ Created {ENV_CUE_FILENAME} with example configuration");
    println!("\nNext steps:");
    println!("  1. Edit {ENV_CUE_FILENAME} to customize your environment");
    println!(
        "  2. Run 'cuenv allow {}' to allow this directory",
        config.working_dir.display()
    );
    println!("  3. Add shell hook with 'eval \"$(cuenv shell init <shell>)\"'");

    Ok(())
}
