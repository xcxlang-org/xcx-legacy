use std::str::FromStr;

use cranelift::codegen::settings::{self, Configurable};

use crate::{Diagnostic, NO_LOCATION};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Profile {
    #[default]
    Debug,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Optimization {
    None,
    Speed,
    Size,
}

impl Optimization {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Speed => "speed",
            Self::Size => "size",
        }
    }

    fn cranelift_level(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Speed => "speed",
            Self::Size => "speed_and_size",
        }
    }
}

impl FromStr for Optimization {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "none" => Ok(Self::None),
            "speed" => Ok(Self::Speed),
            "size" => Ok(Self::Size),
            _ => Err(format!(
                "invalid optimization `{value}`: expected none, speed or size"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cpu {
    #[default]
    Generic,
    Native,
}

impl FromStr for Cpu {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "generic" => Ok(Self::Generic),
            "native" => Ok(Self::Native),
            _ => Err(format!("invalid CPU `{value}`: expected generic or native")),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Emit {
    #[default]
    Executable,
    Object,
}

impl FromStr for Emit {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "exe" => Ok(Self::Executable),
            "obj" => Ok(Self::Object),
            _ => Err(format!("invalid emission `{value}`: expected exe or obj")),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildOptions {
    pub profile: Profile,
    pub optimization: Option<Optimization>,
    pub cpu: Cpu,
    pub emit: Emit,
    pub verify: bool,
    pub frame_pointers: bool,
}

impl BuildOptions {
    pub fn optimization(&self) -> Optimization {
        self.optimization.unwrap_or(match self.profile {
            Profile::Debug => Optimization::None,
            Profile::Release => Optimization::Speed,
        })
    }

    pub fn flags(&self) -> Result<settings::Flags, Diagnostic> {
        let mut builder = settings::builder();
        let debugging = self.profile == Profile::Debug;
        let optimized = self.optimization() != Optimization::None;
        for (name, value) in [
            ("opt_level", self.optimization().cranelift_level()),
            (
                "regalloc_algorithm",
                if optimized {
                    "backtracking"
                } else {
                    "single_pass"
                },
            ),
            (
                "enable_alias_analysis",
                if optimized { "true" } else { "false" },
            ),
            (
                "enable_verifier",
                if debugging || self.verify {
                    "true"
                } else {
                    "false"
                },
            ),
            (
                "regalloc_checker",
                if debugging || self.verify {
                    "true"
                } else {
                    "false"
                },
            ),
            (
                "preserve_frame_pointers",
                if debugging || self.frame_pointers {
                    "true"
                } else {
                    "false"
                },
            ),
            ("is_pic", if cfg!(unix) { "true" } else { "false" }),
        ] {
            builder.set(name, value).map_err(|error| {
                Diagnostic::new(format!("cannot configure `{name}`: {error}"), NO_LOCATION)
            })?;
        }
        Ok(settings::Flags::new(builder))
    }

    pub fn isa(
        &self,
    ) -> Result<std::sync::Arc<dyn cranelift::codegen::isa::TargetIsa>, Diagnostic> {
        cranelift_native::builder_with_options(self.cpu == Cpu::Native)
            .map_err(|message| Diagnostic::new(message, NO_LOCATION))?
            .finish(self.flags()?)
            .map_err(|error| Diagnostic::new(error.to_string(), NO_LOCATION))
    }
}

#[cfg(test)]
mod tests;
