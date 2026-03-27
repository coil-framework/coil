(module
  (import "davenda" "host_call" (func $host_call (param i32 i64) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "__DAVENDA_TYPED_OUTPUT__")
  (func (export "__davenda_typed_output") (result i64)
    i64.const __DAVENDA_TYPED_OUTPUT_PACKED__
  )
  (func (export "__DAVENDA_HANDLER_EXPORT__") (result i32)
    i32.const 6
  )
)
