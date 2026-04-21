use dynamodb_facade::{
    AttributeValue, DynamoDBItem, HasAttribute, IntoAttributeValue, Item, attr_list, dynamodb_item,
    has_attributes,
};
use lambda_appsync::ID;
use serde::Serialize;

use crate::{ItemType, PK, SimpleTable};

lambda_appsync::make_types!("graphql/schema.gql");

impl GameStatus {
    /// The partition key (PK) value used to store the game status in DynamoDB
    pub const PK_TYPE: &'static str = "GAME_STATUS";
    /// The attribute name storing the actual game status value
    pub const PROPERTY_NAME: &'static str = "game_status";
    pub fn to_attribute_value(self) -> AttributeValue {
        self.to_string().into_attribute_value()
    }
    /// Returns the allowed current game status when transitioning to a new status.
    ///
    /// The game status can only transition in a specific order:
    /// - Reset -> Started -> Stopped -> Reset
    ///
    /// This method returns what the current status must be to allow transitioning
    /// to the target status (self).
    pub fn valid_from_status(self) -> Self {
        match self {
            GameStatus::Started => GameStatus::Reset,
            GameStatus::Stopped => GameStatus::Started,
            GameStatus::Reset => GameStatus::Stopped,
        }
    }
}

impl DynamoDBItem<SimpleTable> for GameStatus {
    type AdditionalAttributes = attr_list!(ItemType);
    fn to_item(&self) -> Item<SimpleTable> {
        let minimal_item = Item::minimal_from(self);
        minimal_item.with_attributes([(Self::PROPERTY_NAME.to_owned(), self.to_attribute_value())])
    }

    fn try_from_item(item: Item<SimpleTable>) -> dynamodb_facade::Result<Self> {
        use dynamodb_facade::Error;
        item.get(Self::PROPERTY_NAME)
            .ok_or_else(|| Error::custom("Invalid Schema"))
            .and_then(|a| {
                a.as_s()
                    .map_err(|e| Error::custom(format!("Invalid Schema: {e:?}")))
            })
            .and_then(|s| s.parse().map_err(Error::other))
    }
}
has_attributes! {
    GameStatus {
        PK {
            const VALUE: &'static str = Self::PK_TYPE;
        }
        ItemType {
            const VALUE: &'static str = Self::PK_TYPE;
        }
    }
}

impl Player {
    /// The partition key (PK) prefix used for player items
    pub const PK_TYPE: &'static str = "PLAYER";
}

dynamodb_item! {
    #[table = SimpleTable]
    Player {
        #[partition_key]
        PK {
            fn attribute_id(&self) -> ID {
                self.id
            }
            fn attribute_value(id) -> String {
                format!("{}#{id}", Self::PK_TYPE)
            }
        }
        ItemType {
            const VALUE: &'static str = Self::PK_TYPE;
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerWithSecret<'a> {
    #[serde(flatten)]
    player: &'a Player,
    secret: String,
}
impl<'a> PlayerWithSecret<'a> {
    pub fn new(player: &'a Player, secret: String) -> Self {
        Self { player, secret }
    }
}
dynamodb_item! {
    #[table = SimpleTable]
    PlayerWithSecret<'_> {
        #[partition_key]
        PK {
            fn attribute_id(&self) -> <Player as HasAttribute<PK>>::Id<'id> {
                <Player as HasAttribute<PK>>::attribute_id(self.player)
            }
            fn attribute_value(id) -> <Player as HasAttribute<PK>>::Value {
                <Player as HasAttribute<PK>>::attribute_value(id)
            }
        }
        ItemType {
            const VALUE: &'static str = Player::PK_TYPE;
        }
    }
}
