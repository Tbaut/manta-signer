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

//! Signer Secrets

use crate::config::Config;
use futures::future::BoxFuture;

pub use secrecy::{ExposeSecret, Secret};
pub use subtle::{Choice, ConstantTimeEq, CtOption};

/// Secret Bytes Container
pub type SecretBytes = Secret<Vec<u8>>;

/// Password Secret Wrapper
pub struct Password(CtOption<SecretBytes>);

impl Password {
    /// Builds a new [`Password`] from `password` if `is_known` evaluates to `true`.
    #[inline]
    pub fn new(password: SecretBytes, is_known: Choice) -> Self {
        Self(CtOption::new(password, is_known))
    }

    /// Builds a new [`Password`] from `password`.
    #[inline]
    pub fn from_known(password: SecretBytes) -> Self {
        Self::new(password, 1.into())
    }

    /// Builds a new [`Password`] with a no known value.
    #[inline]
    pub fn from_unknown() -> Self {
        Self::new(Secret::new(Vec::with_capacity(64)), 0.into())
    }

    /// Returns [`Some`] if `self` represents a known password.
    #[inline]
    pub fn known(self) -> Option<SecretBytes> {
        self.0.into()
    }

    /// Returns `true` if `self` represents a known password.
    #[inline]
    pub fn is_known(&self) -> bool {
        self.0.is_some().into()
    }
}

impl Default for Password {
    #[inline]
    fn default() -> Self {
        Self::from_unknown()
    }
}

/// Unit Future
///
/// This `type` is used by the [`setup`], [`wake`], and [`sleep`] methods of [`Authorizer`].
/// See their documentation for more.
///
/// [`setup`]: Authorizer::setup
/// [`wake`]: Authorizer::wake
/// [`sleep`]: Authorizer::sleep
pub type UnitFuture<'t> = BoxFuture<'t, ()>;

/// Password Future
///
/// This `type` is used by the [`password`](Authorizer::password) method of [`Authorizer`].
/// See its documentation for more.
pub type PasswordFuture<'t> = BoxFuture<'t, Password>;

/// Authorizer
pub trait Authorizer {
    /// Prompt Type
    type Prompt;

    /// Message Type
    type Message: Default;

    /// Communication Error Type
    type Error: Default;

    /// Retrieves the password from the authorizer.
    fn password(&mut self) -> PasswordFuture;

    /// Runs some setup for the authorizer using the `config`.
    ///
    /// # Implementation Note
    ///
    /// For custom service implementations, this method should be called before any service is run.
    /// [`Service`] already calls this method internally when running [`Service::serve`].
    ///
    /// [`Service`]: crate::service::Service
    /// [`Service::serve`]: crate::service::Service::serve
    #[inline]
    fn setup<'s>(&'s mut self, config: &'s Config) -> UnitFuture<'s> {
        let _ = config;
        Box::pin(async move {})
    }

    /// Prompts the authorizer with `prompt` so that they can be notified that their password is
    /// requested.
    ///
    /// # Implementation Note
    ///
    /// After [`wake`] is called, [`password`] should be called to retrieve the password. These are
    /// implemented as two separate methods so that [`password`] can be called multiple times for
    /// password retries.
    ///
    /// [`wake`]: Self::wake
    /// [`password`]: Self::password
    #[inline]
    fn wake(&mut self, prompt: Self::Prompt) -> UnitFuture {
        let _ = prompt;
        Box::pin(async move {})
    }

    /// Sends a message to the authorizer to end communication.
    #[inline]
    fn sleep(&mut self, message: Result<Self::Message, Self::Error>) -> UnitFuture {
        let _ = message;
        Box::pin(async move {})
    }

    /// Sends a success message to the authorizer to end communication.
    #[inline]
    fn success(&mut self, message: Self::Message) -> UnitFuture {
        self.sleep(Ok(message))
    }

    /// Sends a failure message to the authorizer to end communication.
    #[inline]
    fn failure(&mut self, error: Self::Error) -> UnitFuture {
        self.sleep(Err(error))
    }
}
