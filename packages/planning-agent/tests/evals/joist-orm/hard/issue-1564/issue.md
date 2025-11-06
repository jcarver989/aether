# IndexManager fails on ReactiveReferences without transformer

I.e. if `Location.ts` has a `Location.group = hasReactiveReference` that is not initted until after the constructor (b/c we don't have the transformer installed in tsx) then this can happen:

```
graphql-1  |   TypeError: Cannot read properties of undefined (reading 'isSet')
graphql-1  |       at getFieldValue (/home/node/app/node_modules/joist-orm/src/IndexManager.ts:139:27)
graphql-1  |       at IndexManager.addEntitiesToIndex (/home/node/app/node_modules/joist-orm/src/IndexManager.ts:113:23)
graphql-1  |       at IndexManager.maybeIndexEntity (/home/node/app/node_modules/joist-orm/src/IndexManager.ts:54:12)
graphql-1  |       at EntityManager.register (/home/node/app/node_modules/joist-orm/src/EntityManager.ts:1339:24)
graphql-1  |       at new BaseEntity (/home/node/app/node_modules/joist-orm/src/BaseEntity.ts:49:10)
graphql-1  |       at new LocationCodegen (/home/node/app/src/entities/codegen/LocationCodegen.ts:459:5)
graphql-1  |       at new Location (/home/node/app/src/entities/Location.ts:31:8)
graphql-1  |       at EntityManager.create (/home/node/app/node_modules/joist-orm/src/EntityManager.ts:684:12)
graphql-1  |       at DataLoader.em.getLoader.cacheKeyFn.cacheKeyFn (/home/node/app/node_modules/joist-orm/src/dataloaders/findOrCreateDataLoader.ts:111:21)
graphql-1  |       at process.processTicksAndRejections (node:internal/process/task_queues:105:5)
graphql-1  |       at findOrCreate (/home/node/app/node_modules/joist-orm/src/EntityManager.ts:673:32)
graphql-1  |       at process.processTicksAndRejections (node:internal/process/task_queues:105:5)
graphql-1  |       at async EntityManager.findOrCreate (/home/node/app/node_modules/joist-orm/src/EntityManager.ts:670:20)
graphql-1  |       at async Location.findOrCreateDpGeneralizedLocationFromPpLocation (/home/node/app/src/entities/Location.ts:267:10)
graphql-1  |       at async copyPlanPackageTlis (/home/node/app/src/jobs/copyTemplateItems/copyPlanPackageToDesignPackage.ts:119:7)
graphql-1  |       at async copyPlanPackageToDesignPackage (/home/node/app/src/jobs/copyTemplateItems/copyPlanPackageToDesignPackage.ts:31:3)
graphql-1  |       at async <anonymous> (/home/node/app/node_modules/joist-orm/src/EntityManager.ts:1308:22)
graphql-1  |       at async <anonymous> (/home/node/app/node_modules/joist-orm/src/drivers/PostgresDriver.ts:87:24)
```
