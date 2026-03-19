use super::*;

#[derive(Clone)]
pub struct DavendaAuth<E> {
    engine: E,
    tenant_id: i64,
}

impl<E> DavendaAuth<E> {
    pub fn new(engine: E, tenant_id: i64) -> Self {
        Self { engine, tenant_id }
    }

    pub fn tenant_id(&self) -> i64 {
        self.tenant_id
    }

    pub fn engine(&self) -> &E {
        &self.engine
    }

    pub fn into_inner(self) -> E {
        self.engine
    }
}

impl<E> DavendaAuth<E>
where
    E: RebacEngine,
{
    pub async fn apply_default_schema(&self) -> Result<(), RebacError> {
        self.apply_model_package(&DefaultAuthModelPackage::default())
            .await
    }

    pub async fn apply_model_package<P>(&self, package: &P) -> Result<(), RebacError>
    where
        P: AuthModelPackage + ?Sized,
    {
        self.engine
            .apply_schema(self.tenant_id, package.schema().clone())
            .await
    }

    pub async fn write(
        &self,
        updates: impl IntoIterator<Item = DefaultTupleUpdate>,
    ) -> Result<(), RebacError> {
        self.engine
            .write_tuples(
                self.tenant_id,
                updates.into_iter().map(Into::into).collect(),
            )
            .await
    }

    pub async fn check(
        &self,
        subject: &DefaultSubject,
        relation: Relation,
        object: &Entity,
    ) -> Result<bool, RebacError> {
        let subject = subject.to_subject();
        let object = object.to_object();

        self.engine
            .check(self.tenant_id, &subject, relation.as_str(), &object)
            .await
    }

    pub async fn check_capability<P>(
        &self,
        package: &P,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
    ) -> Result<bool, DavendaAuthError>
    where
        P: AuthModelPackage + ?Sized,
    {
        let binding = package.resolve_binding(capability, object)?;
        let subject = subject.to_subject();
        let object = object.to_object();

        Ok(self
            .engine
            .check(self.tenant_id, &subject, binding.relation.as_str(), &object)
            .await?)
    }

    pub async fn check_default_capability(
        &self,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
    ) -> Result<bool, DavendaAuthError> {
        self.check_capability(
            &DefaultAuthModelPackage::default(),
            subject,
            capability,
            object,
        )
        .await
    }

    pub async fn explain_capability<P>(
        &self,
        package: &P,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
    ) -> Result<CapabilityExplanation, DavendaAuthError>
    where
        P: AuthModelPackage + ?Sized,
    {
        self.explain_capability_with_options(
            package,
            subject,
            capability,
            object,
            ExplainOptions::default(),
        )
        .await
    }

    pub async fn explain_capability_with_options<P>(
        &self,
        package: &P,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
        options: ExplainOptions,
    ) -> Result<CapabilityExplanation, DavendaAuthError>
    where
        P: AuthModelPackage + ?Sized,
    {
        let tuples = self
            .engine
            .read_tuples(self.tenant_id, None, None, None)
            .await?;

        build_capability_explanation(package, &tuples, subject, capability, object, options)
    }

    pub async fn explain_default_capability(
        &self,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
    ) -> Result<CapabilityExplanation, DavendaAuthError> {
        self.explain_default_capability_with_options(
            subject,
            capability,
            object,
            ExplainOptions::default(),
        )
        .await
    }

    pub async fn explain_default_capability_with_options(
        &self,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
        options: ExplainOptions,
    ) -> Result<CapabilityExplanation, DavendaAuthError> {
        self.explain_capability_with_options(
            &DefaultAuthModelPackage::default(),
            subject,
            capability,
            object,
            options,
        )
        .await
    }

    pub async fn check_many(
        &self,
        requests: impl IntoIterator<Item = AccessCheck>,
    ) -> Result<Vec<bool>, RebacError> {
        self.engine
            .check_many(
                self.tenant_id,
                requests.into_iter().map(Into::into).collect(),
            )
            .await
    }

    pub async fn list_objects(
        &self,
        subject: &DefaultSubject,
        relation: Relation,
        namespace: Namespace,
    ) -> Result<Vec<String>, RebacError> {
        let subject = subject.to_subject();

        self.engine
            .list_objects(
                self.tenant_id,
                &subject,
                relation.as_str(),
                namespace.as_str(),
            )
            .await
    }

    pub async fn list_subject_ids(
        &self,
        object: &Entity,
        relation: Relation,
        namespace: Namespace,
    ) -> Result<Vec<String>, RebacError> {
        let object = object.to_object();

        self.engine
            .list_subjects(
                self.tenant_id,
                &object,
                relation.as_str(),
                namespace.as_str(),
            )
            .await
    }
}
