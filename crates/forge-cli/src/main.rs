use std::fs;
use std::path::Path;

use forge_contracts::{ContractDefinition, ContractId, ContractVersion, ValidatedContract};
use forge_kernel::{NativeBootConfig, boot_native};
use serde_json::Value;

const MAX_JSON_FILE_BYTES: u64 = 1_048_576;
const MAX_HASH_FILE_BYTES: u64 = 268_435_456;

fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let exit_code = match run(&arguments) {
        Ok(()) => 0,
        Err((code, message)) => {
            eprintln!("{message}");
            code
        }
    };
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn run(arguments: &[String]) -> Result<(), (i32, &'static str)> {
    match arguments
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--version"] | ["-V"] => {
            println!("forge {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        ["help"] | ["--help"] | ["-h"] | [] => {
            print_help();
            Ok(())
        }
        ["doctor", "--data-dir", directory] => doctor(Path::new(directory)),
        ["hash", path] => hash_file(Path::new(path)),
        ["contract", "validate", schema_path, instance_path] => {
            validate_contract(Path::new(schema_path), Path::new(instance_path))
        }
        _ => Err((2, "invalid command; run `forge help`")),
    }
}

fn doctor(directory: &Path) -> Result<(), (i32, &'static str)> {
    let boot = boot_native(NativeBootConfig::new(directory))
        .map_err(|_| (1, "native boot validation failed"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&boot.report)
            .map_err(|_| (1, "could not encode boot report"))?
    );
    let forge_kernel::NativeBoot {
        state_store,
        runtime,
        ..
    } = boot;
    drop(state_store);
    runtime.shutdown();
    Ok(())
}

fn hash_file(path: &Path) -> Result<(), (i32, &'static str)> {
    let metadata = fs::metadata(path).map_err(|_| (1, "could not read input file"))?;
    if !metadata.is_file() || metadata.len() > MAX_HASH_FILE_BYTES {
        return Err((2, "input file is not a supported regular file"));
    }
    let bytes = fs::read(path).map_err(|_| (1, "could not read input file"))?;
    let digest = blake3::hash(&bytes).to_hex().to_string();
    println!("{digest}");
    Ok(())
}

fn validate_contract(schema_path: &Path, instance_path: &Path) -> Result<(), (i32, &'static str)> {
    let schema = read_json_file(schema_path)?;
    let instance = read_json_file(instance_path)?;
    let definition = ContractDefinition {
        id: ContractId::new("forge.cli.validate")
            .map_err(|_| (2, "contract identity is invalid"))?,
        version: ContractVersion::new(1, 0, 0),
        owner: "forge.cli".to_owned(),
        schema,
    };
    let contract = ValidatedContract::compile(definition)
        .map_err(|_| (2, "contract schema is invalid or has an external reference"))?;
    match contract.validate(&instance) {
        Ok(()) => {
            println!(
                "{}",
                serde_json::json!({"valid": true, "contractFingerprint": &contract.fingerprint().value})
            );
            Ok(())
        }
        Err(violation) => {
            println!(
                "{}",
                serde_json::json!({"valid": false, "invalidPaths": violation.invalid_paths, "additionalErrorCount": violation.additional_error_count})
            );
            Err((1, "instance does not satisfy the contract"))
        }
    }
}

fn read_json_file(path: &Path) -> Result<Value, (i32, &'static str)> {
    let metadata = fs::metadata(path).map_err(|_| (1, "could not read JSON input"))?;
    if !metadata.is_file() || metadata.len() > MAX_JSON_FILE_BYTES {
        return Err((2, "JSON input exceeds the supported size"));
    }
    let bytes = fs::read(path).map_err(|_| (1, "could not read JSON input"))?;
    serde_json::from_slice(&bytes).map_err(|_| (2, "JSON input is malformed"))
}

fn print_help() {
    println!(
        "Hive Forge CLI\n\nCommands:\n  forge doctor --data-dir <path>\n  forge hash <file>\n  forge contract validate <schema.json> <instance.json>\n  forge --version"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_and_version_are_available_without_runtime_services() {
        assert!(run(&["help".to_owned()]).is_ok());
        assert!(run(&["--version".to_owned()]).is_ok());
        assert!(run(&["doctor".to_owned()]).is_err());
    }
}
