use std::borrow::Cow;

use fs_err as fs;

use crate::{
    auth_cookie::get_auth_cookie,
    options::{GlobalOptions, UploadModelOptions},
    roblox_web_api::{ModelUploadData, RobloxApiClient},
};

pub fn upload_model(global: GlobalOptions, options: UploadModelOptions) {
    let auth = global
        .auth
        .clone()
        .or_else(get_auth_cookie)
        .expect("no auth cookie found");

    let model_data = fs::read(options.path).expect("couldn't read input file");

    let mut client = RobloxApiClient::new(Some(auth));

    let upload_data = ModelUploadData {
        model_data: Cow::Owned(model_data),
        name: &options.name,
        description: &options.description,
        group_id: None,
    };

    let response = client
        .upload_model(upload_data)
        .expect("Roblox API request failed");

    println!("Image uploaded successfully!");
    println!("{}", response.backing_asset_id);
}
