use crate::child::Child;
use crate::config::ApplicationRule;
use crate::window_system::WindowInfo;
use atspi_proxies::accessible::AccessibleProxy;
use zbus::Connection;

/// AT-SPI backend — async D-Bus tree walk.
pub struct AtspiBackend {
    conn: Connection,
    rule: ApplicationRule,
    window_info: WindowInfo,
    scale_factor: f64,
}

impl AtspiBackend {
    /// Create a new AT-SPI backend connected to the accessibility bus.
    pub async fn new(
        window_info: WindowInfo,
        rule: ApplicationRule,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let a11y_conn = atspi_connection::AccessibilityConnection::new().await?;
        let conn = a11y_conn.connection().clone();
        let scale_factor = rule.scale_factor;

        Ok(Self {
            conn,
            rule,
            window_info,
            scale_factor,
        })
    }

    /// Find the active AT-SPI window matching our X11 PID.
    async fn find_active_window(&self) -> Result<Option<AccessibleProxy<'_>>, zbus::Error> {
        log::debug!("Getting desktop proxy...");
        let desktop = AccessibleProxy::builder(&self.conn)
            .destination("org.a11y.atspi.Registry")?
            .path("/org/a11y/atspi/accessible/root")?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;

        log::debug!("Desktop proxy built. Getting app count...");
        let app_count = desktop.child_count().await?;
        log::debug!("App count: {}", app_count);

        for app_idx in 0..app_count {
            log::debug!("Getting app {}/{}", app_idx, app_count);
            let app_ref = desktop.get_child_at_index(app_idx).await?;
            let app_proxy = AccessibleProxy::builder(&self.conn)
                .destination(app_ref.name)?
                .path(app_ref.path)?
                .cache_properties(zbus::proxy::CacheProperties::No)
                .build()
                .await?;

            // Skip mutter frames
            if let Ok(desc) = app_proxy.description().await {
                if desc.contains("mutter-x11-frames") {
                    continue;
                }
            }

            log::debug!("Getting win count for app {}", app_idx);
            let win_count = app_proxy.child_count().await?;
            for win_idx in 0..win_count {
                log::debug!("Getting win {}/{}", win_idx, win_count);
                let win_ref = app_proxy.get_child_at_index(win_idx).await?;
                let win_proxy = AccessibleProxy::builder(&self.conn)
                    .destination(win_ref.name)?
                    .path(win_ref.path)?
                    .cache_properties(zbus::proxy::CacheProperties::No)
                    .build()
                    .await?;

                // Check if window is ACTIVE and matches our PID
                let state = win_proxy.get_state().await.unwrap_or_default();
                let pid = win_proxy
                    .inner()
                    .get_property::<u32>("ProcessId")
                    .await
                    .unwrap_or(0);

                // StateType.ACTIVE = bit 1 in the state set
                let is_active = state.contains(atspi_common::State::Active);

                if is_active && pid == self.window_info.pid {
                    return Ok(Some(win_proxy));
                }
            }
        }

        Ok(None)
    }

    /// Recursively walk the accessibility tree and collect matching children.
    ///
    /// Uses async batching: all children at one level are fetched concurrently
    /// with `futures::join_all`, cutting tree walk from O(n × RTT) to O(depth × RTT).
    async fn walk_children(
        &self,
        proxy: &AccessibleProxy<'_>,
        children: &mut Vec<Child>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let count = proxy.child_count().await?;
        if count == 0 {
            return Ok(());
        }

        // Batch-fetch all children at this level concurrently
        let mut child_futures = Vec::with_capacity(count as usize);
        for i in 0..count {
            child_futures.push(proxy.get_child_at_index(i));
        }

        let child_results: Vec<_> = futures::future::join_all(child_futures).await;

        // Process each child
        for result in child_results {
            let child_ref = match result {
                Ok(v) => v,
                Err(_) => continue,
            };

            let child_proxy = match AccessibleProxy::builder(&self.conn)
                .destination(child_ref.name.clone())
                .and_then(|b| b.path(child_ref.path.clone()))
                .map(|b| b.cache_properties(zbus::proxy::CacheProperties::No))
            {
                Ok(b) => match b.build().await {
                    Ok(p) => p,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            let comp_proxy = match atspi_proxies::component::ComponentProxy::builder(&self.conn)
                .destination(child_ref.name)
                .and_then(|b| b.path(child_ref.path))
                .map(|b| b.cache_properties(zbus::proxy::CacheProperties::No))
            {
                Ok(b) => match b.build().await {
                    Ok(p) => p,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            // Get role and state (batch these too)
            let role_fut = child_proxy.get_role();
            let state_fut = child_proxy.get_state();
            let extents_fut =
                comp_proxy.get_extents(atspi_common::CoordType::Screen);

            let (role_result, state_result, extents_result) =
                tokio::join!(role_fut, state_fut, extents_fut);

            let role = role_result.unwrap_or(atspi_common::Role::Invalid) as i32;
            let state_bits = state_result.unwrap_or_default();

            // Check state match (ALL: sensitive + showing + visible)
            let states_match = self.rule.states.iter().all(|&required_state| {
                let st = match required_state {
                    24 => atspi_common::State::Sensitive,
                    25 => atspi_common::State::Showing,
                    30 => atspi_common::State::Visible,
                    _ => atspi_common::State::Invalid,
                };
                state_bits.contains(st)
            });

            // Check role match (NONE: role NOT in excluded list)
            let role_match = if self.rule.roles_match_type == crate::config::ATSPI_MATCH_NONE {
                !self.rule.roles.contains(&role)
            } else {
                self.rule.roles.contains(&role)
            };

            if states_match && role_match {
                if let Ok(ext) = extents_result {
                    let (ex, ey, ew, eh) = ext;
                    let abs_x = (ex as f64) * self.scale_factor;
                    let abs_y = (ey as f64) * self.scale_factor;
                    let rel_x = abs_x - self.window_info.extents.0 as f64;
                    let rel_y = abs_y - self.window_info.extents.1 as f64;

                    // Skip elements not visible in the window
                    if rel_x >= 0.0 && rel_y >= 0.0 {
                        children.push(Child {
                            relative_position: (rel_x, rel_y),
                            absolute_position: (abs_x, abs_y),
                            width: (ew as f64) * self.scale_factor,
                            height: (eh as f64) * self.scale_factor,
                        });
                    }
                }
            }

            // Recurse into children
            // Box::pin to handle recursive async
            if let Err(e) = Box::pin(self.walk_children(&child_proxy, children)).await {
                log::debug!("Error walking child: {}", e);
            }
        }

        Ok(())
    }

    /// Get all matching accessible children for the focused window.
    pub async fn get_children(&self) -> Result<Vec<Child>, Box<dyn std::error::Error>> {
        let mut children = Vec::new();

        let window = self.find_active_window().await?;
        if let Some(win_proxy) = window {
            self.walk_children(&win_proxy, &mut children).await?;
        }

        if children.is_empty() {
            return Err("No accessible children found".into());
        }

        Ok(children)
    }
}
