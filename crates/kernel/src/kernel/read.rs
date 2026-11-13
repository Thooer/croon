//! Definition closure reads and business reads restricted to consumed result keys.
//! 参考：docs/Kernel/Reference.md（求值中的读取位置）。

use super::*;
use crate::store::Budget;
use std::cell::RefCell;

pub(super) struct Reader<'a> {
    kernel: &'a Kernel,
    instance: &'a str,
    fixed: Option<Rc<Content>>,
    budget: Rc<Budget>,
    code_cache: RefCell<Option<Rc<Content>>>,
    business_cache: RefCell<BTreeMap<Vec<String>, Rc<Content>>>,
}
impl<'a> Reader<'a> {
    pub(super) fn new(kernel: &'a Kernel, instance: &'a str, fixed: Option<Rc<Content>>) -> Self {
        Self {
            kernel,
            instance,
            fixed,
            budget: Budget::new(kernel.config.budgets),
            code_cache: RefCell::new(None),
            business_cache: RefCell::new(BTreeMap::new()),
        }
    }
    fn code(&self) -> Result<Rc<Content>, crate::host::HostError> {
        if let Some(fixed) = &self.fixed {
            return Ok(fixed.clone());
        }
        if let Some(cached) = self.code_cache.borrow().as_ref() {
            return Ok(cached.clone());
        }
        let mut scope = Scope::default();
        scope.bundles.insert(
            self.kernel.index.journals[self.instance].birth()["definition"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        );
        let content = Rc::new(Content::with_budget(
            self.kernel.store.clone(),
            scope,
            self.budget.clone(),
        )?);
        *self.code_cache.borrow_mut() = Some(content.clone());
        Ok(content)
    }
    fn business(&self, keys: &[String]) -> Result<Rc<Content>, crate::host::HostError> {
        if let Some(fixed) = &self.fixed {
            return Ok(fixed.clone());
        }
        let mut keys = keys.to_vec();
        keys.sort();
        keys.dedup();
        if let Some(cached) = self.business_cache.borrow().get(&keys) {
            return Ok(cached.clone());
        }
        let scope = self.kernel.view().scope(self.instance, Some(&keys))?;
        let content = Rc::new(Content::with_budget(
            self.kernel.store.clone(),
            scope,
            self.budget.clone(),
        )?);
        self.business_cache
            .borrow_mut()
            .insert(keys, content.clone());
        Ok(content)
    }
}
impl Read for Reader<'_> {
    fn code_bytes(&self, id: &str) -> Result<Option<Vec<u8>>, crate::host::HostError> {
        self.code()?.bytes(id)
    }
    fn code_json(&self, id: &str) -> Result<Option<Json>, crate::host::HostError> {
        self.code()?.json(id)
    }
    fn code_bundle(&self, id: &str) -> Result<Option<Bundle>, crate::host::HostError> {
        self.code()?.bundle(id)
    }
    fn bytes(
        &self,
        id: &str,
        consumed: &[String],
    ) -> Result<Option<Vec<u8>>, crate::host::HostError> {
        self.business(consumed)?.bytes(id)
    }
    fn json(&self, id: &str, consumed: &[String]) -> Result<Option<Json>, crate::host::HostError> {
        self.business(consumed)?.json(id)
    }
    fn bundle(
        &self,
        id: &str,
        consumed: &[String],
    ) -> Result<Option<Bundle>, crate::host::HostError> {
        self.business(consumed)?.bundle(id)
    }
}
