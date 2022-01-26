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

//! Manta Signer UI

// TODO: Check what the `windows_subsystem` attributes do, and if we need them.

#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![forbid(rustdoc::broken_intra_doc_links)]
#![forbid(missing_docs)]
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use async_std::{fs, path::PathBuf, stream::StreamExt, sync::Arc};
use manta_signer::{
    config::Config,
    secret::{
        account_exists, create_account, Authorizer, ExposeSecret, Password, PasswordFuture,
        SecretString, UnitFuture,
    },
    service::{Prompt, Service},
};
use serde::{Deserialize, Serialize};
use tauri::{
    async_runtime::{channel, spawn, Mutex, Receiver, Sender},
    CustomMenuItem, Event, Manager, State, SystemTray, SystemTrayEvent, SystemTrayMenu, Window,
};

/// User
pub struct User {
    /// Main Window
    window: Window,

    /// Password Receiver
    password: Receiver<Password>,

    /// Password Retry Sender
    retry: Sender<bool>,

    /// Waiting Flag
    waiting: bool,

    /// Resource Directory
    resource_directory: PathBuf,
}

impl User {
    /// Builds a new [`User`] from `window`, `password`, `retry`, and `resource_directory`.
    #[inline]
    pub fn new(
        window: Window,
        password: Receiver<Password>,
        retry: Sender<bool>,
        resource_directory: PathBuf,
    ) -> Self {
        Self {
            window,
            password,
            retry,
            waiting: false,
            resource_directory,
        }
    }

    /// Pulls resources from `self.resource_directory` and moves them to the proving key directory.
    #[inline]
    async fn setup_resources(&self, config: &Config) {
        if !self.resource_directory.exists().await {
            // NOTE: If this file does not exist, then we are in development mode.
            return;
        }
        let mut entries = fs::read_dir(&self.resource_directory)
            .await
            .expect("The resource directory should be a directory.");
        while let Some(entry) = entries.next().await {
            let entry = entry.expect("Unable to get directory entry.");
            if entry.file_type().await.unwrap().is_file() {
                let path = entry.path();
                if matches!(path.extension(), Some(ext) if ext == "bin") {
                    fs::copy(
                        &path,
                        &config
                            .proving_key_directory
                            .join(&path.file_name().expect("Path should point to a real file.")),
                    )
                    .await
                    .expect("Copy should have succeeded.");
                }
            } else if entry.path().to_str().unwrap().ends_with(".bin") {
                let mut bins = fs::read_dir(&entry.path())
                    .await
                    .expect("The resource directory should be a directory.");
                while let Some(entry) = bins.next().await {
                    let path = entry.expect("Unable to get directory entry.").path();
                    fs::copy(
                        &path,
                        &config
                            .proving_key_directory
                            .join(&path.file_name().expect("Path should point to a real file.")),
                    )
                    .await
                    .expect("Copy should have succeeded.");
                }
            }
        }
    }

    /// Emits a `message` of the given `kind` to the window.
    #[inline]
    fn emit<T>(&self, kind: &'static str, message: T)
    where
        T: Serialize,
    {
        self.window.emit(kind, message).unwrap()
    }

    /// Sends a the `retry` message to have the user retry the password.
    #[inline]
    async fn should_retry(&mut self, retry: bool) {
        self.retry
            .send(retry)
            .await
            .expect("Failed to send retry message.");
    }

    /// Requests password from user, sending a retry message if the previous password did not match
    /// correctly.
    #[inline]
    async fn request_password(&mut self) -> Password {
        if self.waiting {
            self.should_retry(true).await;
        }
        let password = self
            .password
            .recv()
            .await
            .expect("Failed to receive retry message.");
        self.waiting = password.is_known();
        password
    }

    /// Sends validation message when password was correctly matched.
    #[inline]
    async fn validate_password(&mut self) {
        self.waiting = false;
        self.should_retry(false).await;
    }
}

impl Authorizer for User {
    type Prompt = Prompt;

    type Message = ();

    type Error = ();

    #[inline]
    fn password(&mut self) -> PasswordFuture {
        Box::pin(async move { self.request_password().await })
    }

    #[inline]
    fn setup<'s>(&'s mut self, config: &'s Config) -> UnitFuture<'s> {
        Box::pin(async move { self.setup_resources(config).await })
    }

    #[inline]
    fn wake(&mut self, prompt: Self::Prompt) -> UnitFuture {
        self.emit("authorize", prompt);
        Box::pin(async move {})
    }

    #[inline]
    fn sleep(&mut self, message: Result<Self::Message, Self::Error>) -> UnitFuture {
        let _ = message;
        Box::pin(async move { self.validate_password().await })
    }
}

/// Password Storage Channel
struct PasswordStoreChannel {
    /// Password Sender
    password: Sender<Password>,

    /// Retry Receiver
    retry: Receiver<bool>,
}

/// Password Storage Type
type PasswordStoreType = Arc<Mutex<Option<PasswordStoreChannel>>>;

/// Password Storage Handle
pub struct PasswordStoreHandle(PasswordStoreType);

impl PasswordStoreHandle {
    /// Constructs the opposite end of `self` for the password storage handle.
    #[inline]
    pub async fn into_channel(self) -> (Receiver<Password>, Sender<bool>) {
        let (password, receiver) = channel(1);
        let (sender, retry) = channel(1);
        *self.0.lock().await = Some(PasswordStoreChannel { password, retry });
        (receiver, sender)
    }
}

/// Password Storage
#[derive(Default)]
pub struct PasswordStore(PasswordStoreType);

impl PasswordStore {
    /// Returns a handle for setting up a [`PasswordStore`].
    #[inline]
    pub fn handle(&self) -> PasswordStoreHandle {
        PasswordStoreHandle(self.0.clone())
    }

    /// Loads the password store with `password`, returning `true` if the password was correct.
    #[inline]
    pub async fn load(&self, password: SecretString) -> bool {
        if let Some(store) = &mut *self.0.lock().await {
            let _ = store.password.send(Password::from_known(password)).await;
            store.retry.recv().await.unwrap()
        } else {
            false
        }
    }

    /// Loads the password with `password`, not requesting a retry.
    #[inline]
    pub async fn load_exact(&self, password: SecretString) {
        if let Some(store) = &mut *self.0.lock().await {
            let _ = store.password.send(Password::from_known(password)).await;
        }
    }

    /// Clears the password from the store.
    #[inline]
    pub async fn clear(&self) {
        if let Some(store) = &mut *self.0.lock().await {
            let _ = store.password.send(Password::from_unknown()).await;
        }
    }
}

/// Sends the current `password` into storage from the UI.
#[tauri::command]
async fn send_password(
    password_store: State<'_, PasswordStore>,
    password: String,
) -> Result<bool, ()> {
    Ok(password_store.load(password.into()).await)
}

/// Stops the server from prompting for the password.
#[tauri::command]
async fn stop_password_prompt(password_store: State<'_, PasswordStore>) -> Result<(), ()> {
    password_store.clear().await;
    Ok(())
}

/// Connection Event
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ConnectEvent {
    /// Create Account
    CreateAccount,

    /// Setup Authorization
    SetupAuthorization,
}

/// Starts the first round of communication between the UI and the signer.
#[tauri::command]
async fn connect(config: State<'_, Config>) -> Result<ConnectEvent, ()> {
    match account_exists(&config.root_seed_file).await {
        Ok(true) => Ok(ConnectEvent::SetupAuthorization),
        _ => Ok(ConnectEvent::CreateAccount),
    }
}

/// Sends the mnemonic to the UI for the user to memorize.
#[tauri::command]
async fn get_mnemonic(
    config: State<'_, Config>,
    password_store: State<'_, PasswordStore>,
    password: String,
) -> Result<String, ()> {
    let password = password.into();
    let mnemonic = create_account(&config.root_seed_file, &password)
        .await
        .map_err(move |_| ())?
        .expose_secret()
        .clone()
        .into_phrase();
    password_store.load_exact(password).await;
    Ok(mnemonic)
}

/// Runs the main Tauri application.
fn main() {
    let config =
        Config::try_default().expect("Unable to generate the default server configuration.");

    let mut app = tauri::Builder::default()
        .system_tray(
            SystemTray::new().with_menu(
                SystemTrayMenu::new()
                    .add_item(CustomMenuItem::new("about", "About"))
                    .add_item(CustomMenuItem::new("exit", "Quit")),
            ),
        )
        .on_system_tray_event(move |app, event| {
            if let SystemTrayEvent::MenuItemClick { id, .. } = event {
                match id.as_str() {
                    "about" => app.get_window("about").unwrap().show().unwrap(),
                    "exit" => app.exit(0),
                    _ => {}
                }
            }
        })
        .manage(PasswordStore::default())
        .manage(config)
        .setup(|app| {
            let resource_directory = app.path_resolver().resource_dir().unwrap();
            let window = app.get_window("main").unwrap();
            let config = app.state::<Config>().inner().clone();
            let password_store = app.state::<PasswordStore>().handle();
            spawn(async move {
                let (password, retry) = password_store.into_channel().await;
                Service::build(
                    config,
                    User::new(window, password, retry, resource_directory.into()),
                )
                .serve()
                .await
                .expect("Unable to build manta-signer service.");
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            connect,
            get_mnemonic,
            send_password,
            stop_password_prompt,
        ])
        .build(tauri::generate_context!())
        .expect("Error while building UI.");

    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);

    app.run(|app, event| match event {
        Event::Ready => app.get_window("about").unwrap().hide().unwrap(),
        Event::CloseRequested { label, api, .. } => {
            api.prevent_close();
            match label.as_str() {
                "about" => app.get_window(&label).unwrap().hide().unwrap(),
                "main" => app.exit(0),
                _ => unreachable!("There are no other windows."),
            }
        }
        _ => (),
    })
}