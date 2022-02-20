// Copyright 2019-2022 Manta Network.
// This file is part of manta-signer.
//
// manta-signer is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// manta-signer is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with manta-signer. If not, see <http://www.gnu.org/licenses/>.

//! Manta Signer Configuration

use manta_crypto::rand::OsRng;
use manta_pay::key::Mnemonic;
use manta_util::serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
};
use tokio::fs;

/// Manta Path Identifier
pub const PATH_IDENTIFIER: &str = "manta-signer";

/// Pushes the [`PATH_IDENTIFIER`] to the end of the given `path` if it exists, attaching the file
/// `name` afterwards.
#[inline]
fn file<P>(path: Option<PathBuf>, name: P) -> Option<PathBuf>
where
    P: AsRef<Path>,
{
    path.map(move |mut p| {
        p.push(PATH_IDENTIFIER);
        p.push(name);
        p
    })
}

/// Configuration
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(crate = "manta_util::serde", deny_unknown_fields)]
pub struct Config {
    /// Data File Path
    pub data_path: PathBuf,

    /// Service URL
    pub service_url: String,

    /// Origin URL
    pub origin_url: String,
}

impl Config {
    /// Tries to build a default [`Config`].
    #[inline]
    pub fn try_default() -> Option<Self> {
        Some(Self {
            data_path: file(dirs_next::config_dir(), "storage.dat")?,
            service_url: String::from("http://127.0.0.1:29987"),
            #[cfg(feature = "unsafe-disable-cors")]
            origin_url: String::from("*"),
            #[cfg(not(feature = "unsafe-disable-cors"))]
            origin_url: String::from("https://app.dolphin.manta.network"),
        })
    }

    ///
    #[inline]
    pub async fn setup(&self) -> io::Result<Setup> {
        match fs::metadata(&self.data_path).await {
            Ok(metadata) if metadata.is_file() => Ok(Setup::Login),
            Ok(metadata) => todo!(),
            _ => Ok(Setup::CreateAccount(Mnemonic::random(
                &mut OsRng,
                Default::default(),
            ))),
        }
    }
}

/// Setup Phase
pub enum Setup {
    /// Create Account
    CreateAccount(Mnemonic),

    /// Login
    Login,
}
