use serde::Serialize;
use serde_json::Value;
use wreq::Client;

use crate::{
    errors::Result,
    models::{RoomKind, RoomReservation},
    session::SessionStore,
};
use super::super::http::{build_headers, ensure_no_error, required_str, ContentType};

const RESERVATIONS_ENDPOINT: &str = "https://eu.mspapis.com/matchmaker/v1/games/j68d/reservations/";

pub struct ReservationsEndpoint<'c> {
    pub(crate) http:    &'c Client,
    pub(crate) session: &'c SessionStore,
}

impl<'c> ReservationsEndpoint<'c> {
    #[tracing::instrument(name = "reserve_chatroom", skip_all, fields(level = %level, version = %version))]
    pub async fn chatroom(&self, level: &str, version: &str) -> Result<RoomReservation> {
        self.reserve(RoomKind::Chatroom, level, version).await
    }

    #[tracing::instrument(name = "reserve_quiz", skip_all)]
    pub async fn quiz(&self) -> Result<RoomReservation> {
        self.reserve(RoomKind::Quiz, "", "624").await
    }

    async fn reserve(
        &self,
        kind: RoomKind,
        level: &str,
        version: &str,
    ) -> Result<RoomReservation> {
        let session = self.session.get().await?;

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Parameters<'a> {
            #[serde(rename = "LoadMode")]  load_mode: &'a str,
            #[serde(rename = "Level")]     level:     &'a str,
            #[serde(rename = "Version")]   version:   &'a str,
            #[serde(rename = "Culture")]   culture:   &'a str,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Payload<'a> {
            join_type:        &'a str,
            room_type:        &'a str,
            room_instance_id: Option<()>,
            parameters:       Parameters<'a>,
        }

        let payload = Payload {
            join_type:        "FindRoomByType",
            room_type:        kind.as_str(),
            room_instance_id: None,
            parameters: Parameters {
                load_mode: "Asset",
                level,
                version,
                culture:   "fr-FR",
            },
        };

        let response: Value = self
            .http
            .post(RESERVATIONS_ENDPOINT)
            .headers(build_headers(ContentType::Json, Some(&session.bearer())))
            .json(&payload)
            .send()
            .await?
            .json()
            .await?;

        let response  = ensure_no_error(response)?;
        let host_url  = required_str(&response, "hostUrl")?.to_owned();
        let room_id   = required_str(&response, "roomId")?.to_owned();

        let socket_url = format!(
            "{}{path}?EIO={eio}&transport=websocket",
            host_url,
            path = kind.socket_path(),
            eio  = kind.eio_version(),
        );

        Ok(RoomReservation { host_url, room_id, socket_url })
    }
}