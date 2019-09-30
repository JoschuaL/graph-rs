use crate::client::*;
use crate::http::IntoResponse;
use crate::http::{Download, GraphResponse};
use crate::http::{FetchClient, UploadSessionClient};
use crate::types::collection::Collection;
use graph_error::{GraphFailure, GraphResult};
use graph_rs_types::complextypes::{ItemPreviewInfo, Thumbnail};
use graph_rs_types::entitytypes::{
    BaseItem, DriveItem, DriveItemVersion, ItemActivity, ThumbnailSet,
};
use handlebars::*;
use reqwest::header::{HeaderValue, CONTENT_LENGTH};
use reqwest::Method;
use serde::export::PhantomData;
use serde_json::json;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

fn template(s: &str, last: &str) -> String {
    if s.starts_with(':') {
        vec!["{{drive_root_path}}{{id}}/", last].join("")
    } else {
        vec!["{{drive_item}}/{{id}}/", last].join("")
    }
}

fn encode(s: &str) -> String {
    if s.starts_with(':') {
        url::percent_encoding::percent_encode(
            s.as_bytes(),
            url::percent_encoding::DEFAULT_ENCODE_SET,
        )
        .collect::<String>()
    } else {
        s.to_string()
    }
}

pub struct DriveRequest<'a, I> {
    client: &'a Graph,
    ident: PhantomData<I>,
}

impl<'a, I> DriveRequest<'a, I> {
    pub fn new(client: &'a Graph) -> DriveRequest<'a, I> {
        let ident = client.ident();
        client.request().registry().register_helper(
            "drive_item",
            Box::new(
                move |_: &Helper,
                      _: &Handlebars,
                      _: &Context,
                      _: &mut RenderContext,
                      out: &mut dyn Output|
                      -> HelperResult {
                    match ident {
                        Ident::Drives => {
                            out.write("items")?;
                        },
                        _ => {
                            out.write("drive/items")?;
                        },
                    }
                    Ok(())
                },
            ),
        );

        client.request().registry().register_helper(
            "drive_root",
            Box::new(
                move |_: &Helper,
                      _: &Handlebars,
                      _: &Context,
                      _: &mut RenderContext,
                      out: &mut dyn Output|
                      -> HelperResult {
                    if ident.ne(&Ident::Drives) {
                        out.write("drive")?;
                    }
                    Ok(())
                },
            ),
        );

        client.request().registry().register_helper(
            "drive_root_path",
            Box::new(
                move |_: &Helper,
                      _: &Handlebars,
                      _: &Context,
                      _: &mut RenderContext,
                      out: &mut dyn Output|
                      -> HelperResult {
                    if ident.ne(&Ident::Drives) {
                        out.write("drive/root")?;
                    } else {
                        out.write("root")?;
                    }
                    Ok(())
                },
            ),
        );

        DriveRequest {
            client,
            ident: PhantomData,
        }
    }
}

impl<'a, I> DriveRequest<'a, I> {
    get!( drive, BaseItem => "{{drive_root}}" );
    get!( root, DriveItem => "{{drive_root}}/root" );
    get!( recent, Collection<DriveItem> => "{{drive_root}}/recent" );
    get!( delta, Collection<DriveItem> => "{{drive_root}}/root/delta" );
    get!( root_children, Collection<DriveItem> => "{{drive_root}}/root/children" );
    get!( | list_children, Collection<DriveItem> => "{{drive_item}}/{{id}}/children" );
    get!( | item_activity, Collection<ItemActivity> => "{{drive_item}}/{{id}}/activities" );
    get!( drive_activity, Collection<ItemActivity> => "{{drive_root}}/activities" );
    get!( thumbnails, Collection<ThumbnailSet> => "{{drive_item}}/thumbnails" );
    get!( shared_with_me, Collection<DriveItem> => "{{drive_root}}/sharedWithMe" );
    get!( special_documents, Collection<DriveItem> => "{{drive_root}}/special/documents" );
    get!( special_documents_children, Collection<DriveItem> => "{{drive_root}}/special/documents/children" );
    get!( special_photos, Collection<DriveItem> => "{{drive_root}}/special/photos" );
    get!( special_photos_children, Collection<DriveItem> => "{{drive_root}}/special/photos/children" );
    get!( special_camera_roll, Collection<DriveItem> => "{{drive_root}}/special/cameraroll" );
    get!( special_camera_roll_children, Collection<DriveItem> => "{{drive_root}}/special/cameraroll/children" );
    get!( special_app_root, Collection<DriveItem> => "{{drive_root}}/special/approot" );
    get!( special_app_root_children, Collection<DriveItem> => "{{drive_root}}/special/approot/children" );
    get!( special_music, Collection<DriveItem> => "{{drive_root}}/special/music" );
    get!( special_music_children, Collection<DriveItem> => "{{drive_root}}/special/music/children" );

    pub fn get_item<S: AsRef<str>>(&'a self, id: S) -> IntoResponse<'a, I, DriveItem> {
        self.client.request().set_method(Method::GET);
        render_path!(
            self.client,
            template(id.as_ref(), "").as_str(),
            &json!({ "id": encode(id.as_ref()) })
        );
        IntoResponse::new(self.client)
    }

    pub fn update<S: AsRef<str>, B: serde::Serialize>(
        &'a self,
        id: S,
        body: &B,
    ) -> IntoResponse<'a, I, DriveItem> {
        self.client
            .request()
            .set_method(Method::PATCH)
            .set_body(serde_json::to_string(body).unwrap());
        render_path!(
            self.client,
            template(id.as_ref(), "").as_str(),
            &json!({"id": encode(id.as_ref()) })
        );
        IntoResponse::new(self.client)
    }

    pub fn delete<S: AsRef<str>>(&'a self, id: S) -> IntoResponse<'a, I, GraphResponse<()>> {
        self.client.request().set_method(Method::DELETE);
        render_path!(
            self.client,
            template(id.as_ref(), "").as_str(),
            &json!({"id": encode(id.as_ref()) })
        );
        IntoResponse::new(self.client)
    }

    pub fn create_folder<S: AsRef<str>>(
        &'a self,
        id: S,
        name: &str,
        conflict_behavior: Option<&str>,
    ) -> IntoResponse<'a, I, DriveItem> {
        let folder: HashMap<String, serde_json::Value> = HashMap::new();
        if let Some(c) = conflict_behavior {
            let data =
                json!({ "name": name, "folder": folder,  "microsoft_graph_conflict_behavior": c });
            self.client
                .request()
                .set_method(Method::POST)
                .set_body(serde_json::to_string(&data).unwrap());
        } else {
            let data = json!({ "name": name, "folder": folder });
            self.client
                .request()
                .set_method(Method::POST)
                .set_body(serde_json::to_string(&data).unwrap());
        }
        render_path!(
            self.client,
            template(id.as_ref(), "children").as_str(),
            &json!({"id": encode(id.as_ref()) })
        );
        IntoResponse::new(self.client)
    }

    pub fn copy<S: AsRef<str>, T: serde::Serialize>(
        &'a self,
        id: S,
        name: Option<&str>,
        item_ref: &T,
    ) -> IntoResponse<'a, I, GraphResponse<()>> {
        if let Some(name) = name {
            let data = json!({ "name": name, "parent_reference": item_ref });
            self.client
                .request()
                .set_method(Method::POST)
                .set_body(serde_json::to_string(&data).unwrap());
        } else {
            let data = json!({ "parent_reference": item_ref });
            self.client
                .request()
                .set_method(Method::POST)
                .set_body(serde_json::to_string(&data).unwrap());
        }
        render_path!(
            self.client,
            template(id.as_ref(), "copy").as_str(),
            &json!({"id": encode(id.as_ref()) })
        );
        IntoResponse::new(self.client)
    }

    pub fn list_versions<S: AsRef<str>>(
        &self,
        id: S,
    ) -> IntoResponse<'a, I, Collection<DriveItemVersion>> {
        render_path!(
            self.client,
            template(id.as_ref(), "versions").as_str(),
            &json!({ "id": encode(id.as_ref()) })
        );
        IntoResponse::new(self.client)
    }

    pub fn single_thumbnail<S: AsRef<str>>(
        &'a self,
        id: S,
        thumb_id: &str,
        size: &str,
    ) -> IntoResponse<'a, I, Thumbnail> {
        render_path!(
            self.client,
            template(id.as_ref(), "thumbnails/{{thumb_id}}/{{size}}").as_str(),
            &json!({
               "id": encode(id.as_ref()),
               "thumb_id": thumb_id,
               "size": size
            })
        );
        IntoResponse::new(self.client)
    }

    pub fn thumbnail_binary<S: AsRef<str>>(
        &'a self,
        id: S,
        thumb_id: &str,
        size: &str,
    ) -> IntoResponse<'a, I, Vec<u8>> {
        render_path!(
            self.client,
            template(id.as_ref(), "thumbnails/{{thumb_id}}/{{size}}/content").as_str(),
            &json!({
               "id": encode(id.as_ref()),
               "thumb_id": thumb_id,
               "size": size
            })
        );
        IntoResponse::new(self.client)
    }

    pub fn upload_replace<ID: AsRef<str>, P: AsRef<Path>>(
        &'a self,
        id: ID,
        file: P,
    ) -> GraphResult<IntoResponse<'a, I, DriveItem>> {
        self.client
            .request()
            .set_method(Method::PUT)
            .set_body(File::open(file)?);
        render_path!(
            self.client,
            template(id.as_ref(), "content").as_str(),
            &json!({"id": encode(id.as_ref()) })
        );
        Ok(IntoResponse::new(self.client))
    }

    pub fn upload_new<ID: AsRef<str>, P: AsRef<Path>>(
        &'a self,
        parent_id: ID,
        file: P,
    ) -> GraphResult<IntoResponse<'a, I, DriveItem>> {
        let name = file
            .as_ref()
            .file_name()
            .ok_or_else(|| GraphFailure::none_err("file_name"))?
            .to_string_lossy()
            .to_string();
        self.client
            .request()
            .set_method(Method::PUT)
            .set_body(File::open(file)?);
        render_path!(
            self.client,
            "{{drive_item}}/{{id}}/{{file_name}}/content",
            &json!({
                "id": parent_id.as_ref(),
                "file_name": name,
            })
        );
        Ok(IntoResponse::new(self.client))
    }

    pub fn restore_version<S: AsRef<str>>(
        &'a self,
        id: S,
        version_id: S,
    ) -> IntoResponse<'a, I, GraphResponse<()>> {
        render_path!(
            self.client,
            template(id.as_ref(), "versions/{{version_id}}/restoreVersion").as_str(),
            &json!({
                "id": encode(id.as_ref()),
                "version_id": version_id.as_ref(),
            })
        );
        IntoResponse::new(self.client)
    }

    pub fn upload_session<S: AsRef<str>, P: AsRef<Path>, B: serde::Serialize>(
        &'a self,
        id: S,
        file: P,
        body: B,
    ) -> IntoResponse<'a, I, UploadSessionClient> {
        self.client
            .request()
            .set_method(Method::POST)
            .set_upload_session(file)
            .set_body(serde_json::to_string(&json!({ "item": body })).unwrap());
        render_path!(
            self.client,
            template(id.as_ref(), "createUploadSession").as_str(),
            &json!({ "id": encode(id.as_ref()) })
        );
        IntoResponse::new(self.client)
    }

    pub fn preview<S: AsRef<str>, B: serde::Serialize>(
        &'a self,
        id: S,
        embeddable_url: Option<&B>,
    ) -> IntoResponse<'a, I, ItemPreviewInfo> {
        if let Some(embeddable_url) = embeddable_url {
            self.client
                .request()
                .set_method(Method::POST)
                .set_body(serde_json::to_string(embeddable_url).unwrap());
        } else {
            self.client
                .request()
                .set_method(Method::POST)
                .header(CONTENT_LENGTH, HeaderValue::from(0));
        }
        render_path!(
            self.client,
            template(id.as_ref(), "preview").as_str(),
            &json!({ "id": encode(id.as_ref()) })
        );
        IntoResponse::new(self.client)
    }

    pub fn download<S: AsRef<str>, P: AsRef<Path>>(
        &'a self,
        id: S,
        directory: P,
    ) -> GraphResult<FetchClient> {
        render_path!(
            self.client,
            template(id.as_ref(), "content").as_str(),
            &json!({ "id": encode(id.as_ref()) })
        );
        self.client
            .request()
            .set_method(Method::GET)
            .download_request
            .set_directory(PathBuf::from(directory.as_ref()));
        self.client.request().download()
    }

    pub fn check_out<S: AsRef<str>>(&'a self, id: S) -> IntoResponse<'a, I, GraphResponse<()>> {
        render_path!(
            self.client,
            template(id.as_ref(), "checkout").as_str(),
            &json!({ "id": encode(id.as_ref()) })
        );
        self.client
            .request()
            .set_method(Method::POST)
            .header(CONTENT_LENGTH, HeaderValue::from(0));
        IntoResponse::new(self.client)
    }

    pub fn check_in<S: AsRef<str>>(
        &'a self,
        id: S,
        check_in_as: Option<&str>,
        comment: Option<&str>,
    ) -> IntoResponse<'a, I, GraphResponse<()>> {
        if let Some(check_in_as) = check_in_as {
            if let Some(comment) = comment {
                self.client.request().set_body(
                    serde_json::to_string_pretty(
                        &json!({ "checkInAs": check_in_as, "comment": comment }),
                    )
                    .unwrap(),
                );
            } else {
                self.client.request().set_body(
                    serde_json::to_string_pretty(&json!({ "checkInAs": check_in_as })).unwrap(),
                );
            }
        } else if let Some(comment) = comment {
            self.client
                .request()
                .set_body(serde_json::to_string_pretty(&json!({ "comment": comment })).unwrap());
        } else {
            self.client
                .request()
                .header(CONTENT_LENGTH, HeaderValue::from(0));
        }
        render_path!(
            self.client,
            template(id.as_ref(), "checkin").as_str(),
            &json!({ "id": encode(id.as_ref()) })
        );
        self.client.request().set_method(Method::POST);
        IntoResponse::new(self.client)
    }
}
