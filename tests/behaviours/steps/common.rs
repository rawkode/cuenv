use crate::behaviours::world::TestWorld;
use cucumber::{given, then, when};
use std::fs;

#[given("I have a temporary test directory")]
async fn setup_temp_dir(world: &mut TestWorld) -> anyhow::Result<()> {
    world.setup_test_dir()?;
    Ok(())
}

#[given("cuenv is installed")]
async fn verify_cuenv_installed(_world: &mut TestWorld) -> anyhow::Result<()> {
    // Verify that cuenv binary exists
    let cuenv_path = std::path::Path::new("target/debug/cuenv");
    if !cuenv_path.exists() {
        anyhow::bail!("cuenv binary not found at target/debug/cuenv");
    }
    Ok(())
}

#[given(regex = r#"I have a CUE file "(.+)" with:"#)]
async fn create_cue_file(
    world: &mut TestWorld,
    filename: String,
    docstring: String,
) -> anyhow::Result<()> {
    // Debug: print what we receive
    eprintln!("Received filename: {}", filename);
    eprintln!("Received docstring: {}", docstring);

    // The docstring content comes as the last parameter from the feature file
    world.write_cue_file(&filename, &docstring)?;
    Ok(())
}

#[given(regex = r#"I have a file "([^"]+)" with content "([^"]+)""#)]
async fn create_file(
    world: &mut TestWorld,
    filename: String,
    content: String,
) -> anyhow::Result<()> {
    let path = world
        .working_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No working directory set"))?
        .join(filename);
    fs::write(path, content)?;
    Ok(())
}

#[given(regex = r#"I have a subdirectory "([^"]+)""#)]
async fn create_subdirectory(world: &mut TestWorld, dirname: String) -> anyhow::Result<()> {
    let path = world
        .working_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No working directory set"))?
        .join(dirname);
    fs::create_dir_all(path)?;
    Ok(())
}

#[given(regex = r#"^I have a CUE file "([^"]+)" in "([^"]+)" with:$"#)]
async fn create_cue_file_in_dir(
    world: &mut TestWorld,
    filename: String,
    dirname: String,
    docstring: String,
) -> anyhow::Result<()> {
    let dir_path = world
        .working_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No working directory set"))?
        .join(&dirname);
    fs::create_dir_all(&dir_path)?;
    let file_path = dir_path.join(&filename);
    fs::write(file_path, &docstring)?;
    Ok(())
}

#[when(regex = r#"I run "([^"]+)""#)]
async fn run_command(world: &mut TestWorld, command: String) -> anyhow::Result<()> {
    let args: Vec<&str> = command.split_whitespace().collect();
    if args[0] == "cuenv" {
        world.run_cuenv(&args[1..]).await?;
    } else if args[0] == "printenv" {
        world.run_printenv(&args[1..]).await?;
    } else {
        anyhow::bail!(
            "Only cuenv and printenv commands are supported, got: {}",
            args[0]
        );
    }
    Ok(())
}

#[when(regex = r#"I run "([^"]+)" in "([^"]+)""#)]
async fn run_command_in_dir(
    world: &mut TestWorld,
    command: String,
    dir: String,
) -> anyhow::Result<()> {
    let original_dir = world.working_dir.clone();
    let new_dir = world
        .working_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No working directory set"))?
        .join(dir);

    world.working_dir = Some(new_dir);
    let args: Vec<&str> = command.split_whitespace().collect();
    if args[0] == "cuenv" {
        world.run_cuenv(&args[1..]).await?;
    }
    world.working_dir = original_dir;
    Ok(())
}

#[then(regex = r#"the output should contain "([^"]+)""#)]
fn check_output_contains(world: &mut TestWorld, expected: String) -> anyhow::Result<()> {
    let output = world
        .last_output
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No output to check"))?;
    if !output.contains(&expected) {
        anyhow::bail!(
            "Output does not contain '{}'\nActual output: {}",
            expected,
            output
        );
    }
    Ok(())
}

#[then(regex = r#"the output should not contain "([^"]+)""#)]
fn check_output_not_contains(world: &mut TestWorld, unexpected: String) -> anyhow::Result<()> {
    let output = world
        .last_output
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No output to check"))?;
    if output.contains(&unexpected) {
        anyhow::bail!(
            "Output contains '{}' but shouldn't\nActual output: {}",
            unexpected,
            output
        );
    }
    Ok(())
}

#[then("the exit code should be 0")]
fn check_exit_code_zero(world: &mut TestWorld) -> anyhow::Result<()> {
    let exit_code = world
        .last_exit_code
        .ok_or_else(|| anyhow::anyhow!("No exit code to check"))?;
    if exit_code != 0 {
        anyhow::bail!("Exit code was {} instead of 0", exit_code);
    }
    Ok(())
}

#[then("the exit code should not be 0")]
fn check_exit_code_nonzero(world: &mut TestWorld) -> anyhow::Result<()> {
    let exit_code = world
        .last_exit_code
        .ok_or_else(|| anyhow::anyhow!("No exit code to check"))?;
    if exit_code == 0 {
        anyhow::bail!("Exit code was 0 but should be non-zero");
    }
    Ok(())
}

#[then(regex = r#"the error should contain "([^"]+)""#)]
fn check_error_contains(world: &mut TestWorld, expected: String) -> anyhow::Result<()> {
    let error = world
        .last_error
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No error output to check"))?;
    if !error.contains(&expected) {
        anyhow::bail!(
            "Error does not contain '{}'\nActual error: {}",
            expected,
            error
        );
    }
    Ok(())
}

#[then(regex = r#"a "([^"]+)" file should exist"#)]
fn check_file_exists(world: &mut TestWorld, filepath: String) -> anyhow::Result<()> {
    let path = world
        .working_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No working directory set"))?
        .join(filepath);
    if !path.exists() {
        anyhow::bail!("File {} does not exist", path.display());
    }
    Ok(())
}

#[then(regex = r#"the output should match pattern "([^"]+)""#)]
fn check_output_pattern(world: &mut TestWorld, pattern: String) -> anyhow::Result<()> {
    let output = world
        .last_output
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No output to check"))?;
    let regex = regex::Regex::new(&pattern)?;
    if !regex.is_match(output) {
        anyhow::bail!(
            "Output does not match pattern '{}'\nActual output: {}",
            pattern,
            output
        );
    }
    Ok(())
}

#[given("I have allowed the current directory")]
async fn allow_current_directory(world: &mut TestWorld) -> anyhow::Result<()> {
    world.run_cuenv(&["env", "allow"]).await?;
    Ok(())
}

#[when("I change to parent directory")]
async fn change_to_parent_directory(world: &mut TestWorld) -> anyhow::Result<()> {
    let parent = world
        .working_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No working directory set"))?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("No parent directory"))?
        .to_path_buf();
    world.working_dir = Some(parent);
    Ok(())
}

#[when(regex = r#"I change to "([^"]+)" directory"#)]
async fn change_to_named_directory(world: &mut TestWorld, dirname: String) -> anyhow::Result<()> {
    let new_dir = world
        .working_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No working directory set"))?
        .join(dirname);

    if !new_dir.exists() {
        anyhow::bail!("Directory {} does not exist", dirname);
    }

    world.working_dir = Some(new_dir);
    Ok(())
}

#[then(regex = r#"the environment should have "([^"]+)" set to "([^"]+)""#)]
fn check_env_var_set(
    world: &mut TestWorld,
    var_name: String,
    expected_value: String,
) -> anyhow::Result<()> {
    // The shell load command outputs export statements
    let output = world
        .last_output
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No output to check"))?;

    let expected_export = format!("export {}={}", var_name, expected_value);
    if !output.contains(&expected_export) {
        anyhow::bail!(
            "Environment variable {} not set to '{}'\nActual output: {}",
            var_name,
            expected_value,
            output
        );
    }
    Ok(())
}

#[then(regex = r#"the environment should not have "([^"]+)""#)]
fn check_env_var_unset(world: &mut TestWorld, var_name: String) -> anyhow::Result<()> {
    // The shell hook command outputs unset statements when variables should be removed
    let output = world
        .last_output
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("No output to check"))?;

    let unset_command = format!("unset {}", var_name);
    if !output.contains(&unset_command) {
        // Check that the variable is not being exported
        let export_pattern = format!("export {}=", var_name);
        if output.contains(&export_pattern) {
            anyhow::bail!(
                "Environment variable {} is being set when it should be unset\nActual output: {}",
                var_name,
                output
            );
        }
    }
    Ok(())
}

#[then(regex = r#"the command should complete within (\d+) seconds"#)]
fn check_command_duration(world: &mut TestWorld, max_seconds: u64) -> anyhow::Result<()> {
    let duration = world
        .last_command_duration
        .ok_or_else(|| anyhow::anyhow!("No command duration to check"))?;

    let max_duration = std::time::Duration::from_secs(max_seconds);
    if duration > max_duration {
        anyhow::bail!(
            "Command took {:.2}s but should complete within {}s",
            duration.as_secs_f64(),
            max_seconds
        );
    }
    Ok(())
}

#[when(regex = r#"I wait for (\d+) seconds"#)]
async fn wait_for_seconds(_world: &mut TestWorld, seconds: u64) -> anyhow::Result<()> {
    let duration = std::time::Duration::from_secs(seconds);
    tokio::time::sleep(duration).await;
    Ok(())
}
