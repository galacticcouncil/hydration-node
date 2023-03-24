use platforms::*;
use std::{borrow::Cow, env, fs, io, path, process::Command};

/// Generate the `cargo:` key output
pub fn generate_cargo_keys(runtime: &str) -> io::Result<()> {
	let output = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output();

	let commit = match output {
		Ok(o) if o.status.success() => {
			let sha = String::from_utf8_lossy(&o.stdout).trim().to_owned();
			Cow::from(sha)
		}
		Ok(o) => {
			println!("cargo:warning=Git command failed with status: {}", o.status);
			Cow::from("unknown")
		}
		Err(err) => {
			println!("cargo:warning=Failed to execute git command: {err}");
			Cow::from("unknown")
		}
	};

	println!(
		"cargo:rustc-env=SUBSTRATE_CLI_IMPL_VERSION={}",
		get_version(&commit, runtime).unwrap()
	);
	Ok(())
}

fn get_platform() -> String {
	let env_dash = if TARGET_ENV.is_some() { "-" } else { "" };

	format!(
		"{}-{}{}{}",
		TARGET_ARCH.as_str(),
		TARGET_OS.as_str(),
		env_dash,
		TARGET_ENV.map(|x| x.as_str()).unwrap_or(""),
	)
}

fn get_release_version() -> String {
	let output = Command::new("git")
		.args(["describe", "--tags", "--abbrev=0", "--always"])
		.output();

	let version = match output {
		Ok(o) if o.status.success() => {
			let version = String::from_utf8_lossy(&o.stdout).trim().get(1..).unwrap().to_owned();
			Cow::from(version)
		}
		Ok(o) => {
			println!("cargo:warning=Git describe command failed with status: {}", o.status);
			Cow::from("unknown")
		}
		Err(err) => {
			println!("cargo:warning=Failed to execute git describe command: {err}");
			Cow::from("unknown")
		}
	};
	version.to_string()
}

fn get_build_deps(manifest_location: &path::Path) -> io::Result<Vec<(String, String)>> {
	let lock_buf = fs::read_to_string(manifest_location.join("..").join("Cargo.lock"))?;
	Ok(parse_dependencies(&lock_buf))
}

fn parse_dependencies(lock_toml_buf: &str) -> Vec<(String, String)> {
	let lockfile: cargo_lock::Lockfile = lock_toml_buf.parse().expect("Failed to parse lockfile");
	let mut deps = Vec::new();

	for package in lockfile.packages {
		deps.push((package.name.to_string(), package.version.to_string()));
	}
	deps.sort_unstable();
	deps
}

fn get_version(impl_commit: &str, runtime: &str) -> io::Result<String> {
	let commit_dash = if impl_commit.is_empty() { "" } else { "-" };
	let deps = get_build_deps(env::var("CARGO_MANIFEST_DIR").unwrap().as_ref())?;
	let runtime_dependency: Vec<(String, String)> = deps.into_iter().filter(|(dep, _)| dep.eq(runtime)).collect();
	let runtime_version = if runtime_dependency.is_empty() {
		println!("cargo:warning={runtime} found in dependencies");
		"unknown".to_string()
	} else {
		runtime_dependency[0].1.clone()
	};

	Ok(format!(
		"{}{}{} runtime {} node {} {}",
		get_release_version(),
		commit_dash,
		impl_commit,
		runtime_version,
		std::env::var("CARGO_PKG_VERSION").unwrap_or_default(),
		get_platform(),
	))
}
