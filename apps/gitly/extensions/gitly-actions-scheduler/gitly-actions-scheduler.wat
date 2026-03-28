(module
  (import "davenda" "host_call" (func $host_call (param i32 i64) (result i32)))
  (func (export "__DAVENDA_HANDLER_EXPORT__") (result i32)
    i32.const 3
  )
)
