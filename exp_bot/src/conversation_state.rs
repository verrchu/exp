use std::{collections::HashMap, sync::Arc};

use chrono::NaiveDate;
use teloxide_core::types::{MessageId, UserId};
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConversationState {
    AwaitingCategoryName,
    AwaitingCategoryNameConfirmation {
        msg_id: MessageId,
        category_name: String,
    },
    AwaitingExpenseDate {
        msg_id: MessageId,
        category_name: String,
    },
    AwaitingExpenseAmount {
        category_name: String,
        date: NaiveDate,
    },
}

#[derive(Default)]
pub struct ConversationStates {
    inner: Arc<RwLock<HashMap<UserId, ConversationState>>>,
}

impl ConversationStates {
    pub async fn set(&self, user_id: UserId, state: ConversationState) {
        tracing::debug!(
            user_id = user_id.0,
            state = debug(&state),
            "set conversation state"
        );

        self.inner.write().await.insert(user_id, state);
    }

    pub async fn get(&self, user_id: UserId) -> Option<ConversationState> {
        self.inner.read().await.get(&user_id).cloned()
    }

    pub async fn clear(&self, user_id: UserId) {
        self.inner.write().await.remove(&user_id);
    }
}
