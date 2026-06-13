//! Three-tier persistent cache for GLW events.
//!
//! Layered identically to `sl_map_apis::region::RegionNameToGridCoordinatesCache`:
//!
//! 1. In-memory `lru::LruCache` keyed by [`EventId`] or [`GlwEventKey`].
//! 2. On-disk `redb` database holding the JSON-serialised event plus
//!    its `http_cache_semantics::CachePolicy`.
//! 3. Live HTTP fetch via [`crate::client`], with cache-policy
//!    revalidation against the disk-cached policy.

use std::path::PathBuf;

use redb::ReadableDatabase as _;

use crate::client::{default_base_url, fetch_event_by_id, fetch_event_by_key};
use crate::error::GlwEventCacheError;
use crate::types::{EventId, GlwEvent, GlwEventKey};

/// Cached value alongside its `http-cache-semantics` policy.
type CachedEvent = (Option<GlwEvent>, http_cache_semantics::CachePolicy);

/// Redb table mapping numeric event ids to JSON-serialised
/// `Option<GlwEvent>` strings.
const GLW_EVENT_BY_ID_TABLE: redb::TableDefinition<u32, String> =
    redb::TableDefinition::new("glw_event_by_id");

/// Redb table mapping numeric event ids to JSON-serialised
/// `CachePolicy` strings.
const GLW_EVENT_BY_ID_POLICY_TABLE: redb::TableDefinition<u32, String> =
    redb::TableDefinition::new("glw_event_by_id_policy");

/// Redb table mapping string event keys to JSON-serialised
/// `Option<GlwEvent>` strings.
const GLW_EVENT_BY_KEY_TABLE: redb::TableDefinition<String, String> =
    redb::TableDefinition::new("glw_event_by_key");

/// Redb table mapping string event keys to JSON-serialised
/// `CachePolicy` strings.
const GLW_EVENT_BY_KEY_POLICY_TABLE: redb::TableDefinition<String, String> =
    redb::TableDefinition::new("glw_event_by_key_policy");

/// Three-tier cache for GLW events.
///
/// `&mut self` is required on the read methods because the in-memory
/// LRU promotes entries on access and the redb writes happen on the
/// write path.
#[expect(
    clippy::module_name_repetitions,
    reason = "GlwEventCache is the primary public type of this module"
)]
#[derive(Debug)]
pub struct GlwEventCache {
    /// reqwest HTTP client for upstream fetches.
    client: reqwest::Client,
    /// Base URL the `glwDataReq.php` path is joined onto.
    base_url: url::Url,
    /// Persistent on-disk cache (`redb`).
    db: redb::Database,
    /// In-memory cache keyed by numeric event id.
    memory_by_id: lru::LruCache<EventId, CachedEvent>,
    /// In-memory cache keyed by string event key.
    memory_by_key: lru::LruCache<GlwEventKey, CachedEvent>,
}

impl GlwEventCache {
    /// Create a new cache backed by `<cache_directory>/glw_event.redb`.
    ///
    /// `base_url` is the GLW base URL (must include the version segment
    /// as a path, e.g. `http://example.com/glw127/`). Pass `None` to use
    /// the workspace default.
    ///
    /// # Errors
    ///
    /// Returns [`GlwEventCacheError`] if the database file cannot be
    /// created or if the default URL fails to parse (the latter being
    /// unreachable for the hard-coded defaults).
    pub fn new(
        cache_directory: PathBuf,
        base_url: Option<url::Url>,
    ) -> Result<Self, GlwEventCacheError> {
        let client = crate::client::build_http_client();
        let base_url = match base_url {
            Some(u) => u,
            None => default_base_url().map_err(GlwEventCacheError::FetchError)?,
        };
        let db = redb::Database::create(cache_directory.join("glw_event.redb"))?;
        Ok(Self {
            client,
            base_url,
            db,
            memory_by_id: lru::LruCache::unbounded(),
            memory_by_key: lru::LruCache::unbounded(),
        })
    }

    /// Get a GLW event by numeric id.
    ///
    /// Returns `Ok(None)` if the server reports no such event.
    ///
    /// # Errors
    ///
    /// Returns [`GlwEventCacheError`] if a database or upstream HTTP
    /// operation fails.
    #[tracing::instrument(skip(self))]
    pub async fn get_event_by_id(
        &mut self,
        id: EventId,
    ) -> Result<Option<GlwEvent>, GlwEventCacheError> {
        tracing::debug!("Retrieving GLW event by id {id}");
        let cached = self.load_by_id_from_caches(id)?;
        let (event, cache_policy) =
            fetch_event_by_id(&self.client, &self.base_url, id, cached).await?;
        self.store_by_id(id, event.as_ref(), &cache_policy)?;
        Ok(event)
    }

    /// Get a GLW event by string event key.
    ///
    /// # Errors
    ///
    /// Returns [`GlwEventCacheError`] if a database or upstream HTTP
    /// operation fails.
    #[tracing::instrument(skip(self))]
    pub async fn get_event_by_key(
        &mut self,
        key: &GlwEventKey,
    ) -> Result<Option<GlwEvent>, GlwEventCacheError> {
        tracing::debug!("Retrieving GLW event by key {key}");
        let cached = self.load_by_key_from_caches(key)?;
        let (event, cache_policy) =
            fetch_event_by_key(&self.client, &self.base_url, key, cached).await?;
        self.store_by_key(key, event.as_ref(), &cache_policy)?;
        Ok(event)
    }

    /// Force a fresh fetch of an event by id, bypassing all cache tiers.
    ///
    /// # Errors
    ///
    /// Returns [`GlwEventCacheError`] if a database or upstream HTTP
    /// operation fails.
    pub async fn refresh_event_by_id(
        &mut self,
        id: EventId,
    ) -> Result<Option<GlwEvent>, GlwEventCacheError> {
        let (event, cache_policy) =
            fetch_event_by_id(&self.client, &self.base_url, id, None).await?;
        self.store_by_id(id, event.as_ref(), &cache_policy)?;
        Ok(event)
    }

    /// Force a fresh fetch of an event by key, bypassing all cache tiers.
    ///
    /// # Errors
    ///
    /// Returns [`GlwEventCacheError`] if a database or upstream HTTP
    /// operation fails.
    pub async fn refresh_event_by_key(
        &mut self,
        key: &GlwEventKey,
    ) -> Result<Option<GlwEvent>, GlwEventCacheError> {
        let (event, cache_policy) =
            fetch_event_by_key(&self.client, &self.base_url, key, None).await?;
        self.store_by_key(key, event.as_ref(), &cache_policy)?;
        Ok(event)
    }

    /// Look up a cached id-keyed value+policy across the memory and
    /// disk tiers.
    fn load_by_id_from_caches(
        &mut self,
        id: EventId,
    ) -> Result<Option<CachedEvent>, GlwEventCacheError> {
        if let Some(hit) = self.memory_by_id.get(&id) {
            return Ok(Some(hit.to_owned()));
        }
        let read_txn = self.db.begin_read()?;
        let policy = match read_txn.open_table(GLW_EVENT_BY_ID_POLICY_TABLE) {
            Ok(table) => match table.get(id.get())? {
                Some(value) => {
                    let policy: http_cache_semantics::CachePolicy =
                        serde_json::from_str(&value.value())?;
                    Some(policy)
                }
                None => None,
            },
            Err(_) => None,
        };
        let Some(policy) = policy else {
            return Ok(None);
        };
        let value = match read_txn.open_table(GLW_EVENT_BY_ID_TABLE) {
            Ok(table) => match table.get(id.get())? {
                Some(value) => {
                    let event: Option<GlwEvent> = serde_json::from_str(&value.value())?;
                    event
                }
                None => None,
            },
            Err(_) => None,
        };
        Ok(Some((value, policy)))
    }

    /// Look up a cached key-keyed value+policy across the memory and
    /// disk tiers.
    fn load_by_key_from_caches(
        &mut self,
        key: &GlwEventKey,
    ) -> Result<Option<CachedEvent>, GlwEventCacheError> {
        if let Some(hit) = self.memory_by_key.get(key) {
            return Ok(Some(hit.to_owned()));
        }
        let read_txn = self.db.begin_read()?;
        let policy = match read_txn.open_table(GLW_EVENT_BY_KEY_POLICY_TABLE) {
            Ok(table) => match table.get(key.as_str().to_owned())? {
                Some(value) => {
                    let policy: http_cache_semantics::CachePolicy =
                        serde_json::from_str(&value.value())?;
                    Some(policy)
                }
                None => None,
            },
            Err(_) => None,
        };
        let Some(policy) = policy else {
            return Ok(None);
        };
        let value = match read_txn.open_table(GLW_EVENT_BY_KEY_TABLE) {
            Ok(table) => match table.get(key.as_str().to_owned())? {
                Some(value) => {
                    let event: Option<GlwEvent> = serde_json::from_str(&value.value())?;
                    event
                }
                None => None,
            },
            Err(_) => None,
        };
        Ok(Some((value, policy)))
    }

    /// Persist (or evict) the result of an id-keyed lookup. Stores both
    /// positive and negative responses, but only when the policy is
    /// `is_storable` per `http-cache-semantics`.
    fn store_by_id(
        &mut self,
        id: EventId,
        event: Option<&GlwEvent>,
        cache_policy: &http_cache_semantics::CachePolicy,
    ) -> Result<(), GlwEventCacheError> {
        let write_txn = self.db.begin_write()?;
        if cache_policy.is_storable() {
            {
                let mut policy_table = write_txn.open_table(GLW_EVENT_BY_ID_POLICY_TABLE)?;
                policy_table.insert(id.get(), serde_json::to_string(cache_policy)?)?;
            }
            {
                let mut value_table = write_txn.open_table(GLW_EVENT_BY_ID_TABLE)?;
                value_table.insert(id.get(), serde_json::to_string(&event)?)?;
            }
            write_txn.commit()?;
            self.memory_by_id
                .put(id, (event.cloned(), cache_policy.clone()));
        } else {
            tracing::debug!("GLW event by id is not storable; evicting any stale entry");
            {
                let mut policy_table = write_txn.open_table(GLW_EVENT_BY_ID_POLICY_TABLE)?;
                policy_table.remove(id.get())?;
            }
            {
                let mut value_table = write_txn.open_table(GLW_EVENT_BY_ID_TABLE)?;
                value_table.remove(id.get())?;
            }
            write_txn.commit()?;
            self.memory_by_id.pop(&id);
        }
        Ok(())
    }

    /// Persist (or evict) the result of a key-keyed lookup.
    fn store_by_key(
        &mut self,
        key: &GlwEventKey,
        event: Option<&GlwEvent>,
        cache_policy: &http_cache_semantics::CachePolicy,
    ) -> Result<(), GlwEventCacheError> {
        let write_txn = self.db.begin_write()?;
        if cache_policy.is_storable() {
            {
                let mut policy_table = write_txn.open_table(GLW_EVENT_BY_KEY_POLICY_TABLE)?;
                policy_table.insert(
                    key.as_str().to_owned(),
                    serde_json::to_string(cache_policy)?,
                )?;
            }
            {
                let mut value_table = write_txn.open_table(GLW_EVENT_BY_KEY_TABLE)?;
                value_table.insert(key.as_str().to_owned(), serde_json::to_string(&event)?)?;
            }
            write_txn.commit()?;
            self.memory_by_key
                .put(key.to_owned(), (event.cloned(), cache_policy.clone()));
        } else {
            tracing::debug!("GLW event by key is not storable; evicting any stale entry");
            {
                let mut policy_table = write_txn.open_table(GLW_EVENT_BY_KEY_POLICY_TABLE)?;
                policy_table.remove(key.as_str().to_owned())?;
            }
            {
                let mut value_table = write_txn.open_table(GLW_EVENT_BY_KEY_TABLE)?;
                value_table.remove(key.as_str().to_owned())?;
            }
            write_txn.commit()?;
            self.memory_by_key.pop(key);
        }
        Ok(())
    }
}
