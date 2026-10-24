# ANS reference tests

`tester.fr` and `core.fr` are John Hayes' ANS Forth core tests, as maintained by Gerry Jackson in
[forth2012-test-suite](https://github.com/gerryjackson/forth2012-test-suite) (`src/`), unmodified.
Their notice: "(C) 1995 Johns Hopkins University / Applied Physics Laboratory. May be distributed
freely as long as this copyright notice remains."

`cargo test` runs them (`reference_core_fr` in `tests/forth.rs`). By hand:

    include tests/ans/tester.fr
    include tests/ans/core.fr
    #errors ?
