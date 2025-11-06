# Add em.fork

* Return a new `em` with a copy of all currently-loaded entities
* Already-loaded relations should maintain their loaded state
  * I.e. if `author.populate({ books: "reviews" })` was run in `em`, the forked em should see `author.books.isLoaded`, `author.books[0].reviews.isLoaded`, etc.

---

The relationship loaded would probably "just work" if we leverage the preloading cache:

```ts
  private getPreloaded(): U[] | undefined {
    if (this.entity.isNewEntity) return undefined;
    return getEmInternalApi(this.entity.em).getPreloadedRelation<U>(this.entity.idTagged, this.fieldName);
  }
```

I.e. if the parent EM was able to shove/encode all loaded relations into the `a:123.books => [b:1, b:2]` cache, then whenever `a.books.isLoaded` runs, it would just find it.
