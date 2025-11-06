# EntityManager.load/loadAll should runtime check subtypes

I.e. doing `em.load(SubTypeOne, "base:someIdThatIsReallySubTypeTwo")` and `loadAll` will "correctly" create an entity that is `SubTypeTwo`, but we should fail with a runtime error that the  user asked for "the wrong type".
