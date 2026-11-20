- **The arid rules have the Java edition's ids.** A statement that only writes output is
  `mutastic:output`, and a removed `x.unwrap();` that only checks the value is there is
  `mutastic:null-checks`, as Java's `Objects.requireNonNull(x)` is. The editions' contract lists
  every rule, and each has a section on the [Arid rules](https://docs.mutastic.io/reference/arid/)
  page.
