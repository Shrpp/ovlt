use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "authorization_codes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub code: String,
    pub tenant_id: Uuid,
    #[sea_orm(column_type = "Text")]
    pub client_id: String,
    pub user_id: Uuid,
    #[sea_orm(column_type = "Text")]
    pub redirect_uri: String,
    #[sea_orm(column_type = "Text")]
    pub scopes: String,
    #[sea_orm(column_type = "Text")]
    pub code_challenge: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub nonce: Option<String>,
    pub expires_at: DateTimeWithTimeZone,
    pub used_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
