;; Source of the wasm module embedded (base64) in check-v8-shared-memory-grow.mjs.
;; See that script and internal-docs/wasi-shared-memory-grow/design.md.
;;
;; Rebuild (wabt 1.0.42) and re-embed:
;;   wat2wasm --enable-threads scripts/wasi/check-v8-shared-memory-grow.wat -o probe.wasm
;;   base64 -i probe.wasm        # paste into HANDOFF_WASM_BASE64 in the .mjs
;; The committed build is 462 bytes, sha256
;; 993be47a94826b70c5de2b4f0bcbfe4423615e9be59188d88024c61708a661e6.
;;
;; Page 0 layout (all i32, the memory is a SharedArrayBuffer):
;;   [0]  mailbox: pointer into the pages A just grew (0 = none yet)
;;   [4]  ack: last round B finished
;;   [8]  B stage: 1 memory.copy, 2 memory.fill, 3 atomic rmw, 0 idle
;;   [12] B round
;;   [16] B is inside its long activation (set from wasm)
;;   [24] abort (set by B's JS catch so A stops waiting)
;;   [36] value B's refresh returned this round (memory.grow(0) or memory.size, in pages)
;;   [64..80) copy source,  [1024..1040) warm-up target
(module
  (import "env" "memory" (memory 1 65536 shared))

  ;; Worker A: per round grow by one page, write into the new page with a plain store,
  ;; publish a pointer into it, wait for B's ack. Returns rounds done, -1 grow failed, -2 abort.
  (func (export "grow_loop") (param $rounds i32) (result i32)
    (local $r i32) (local $ptr i32) (local $old i32)
    (i32.store (i32.const 64) (i32.const 0x11111111))
    (i32.store (i32.const 68) (i32.const 0x22222222))
    (i32.store (i32.const 72) (i32.const 0x33333333))
    (i32.store (i32.const 76) (i32.const 0x44444444))
    (block $out
      (loop $round
        (br_if $out (i32.ge_u (local.get $r) (local.get $rounds)))
        (local.set $r (i32.add (local.get $r) (i32.const 1)))
        (local.set $old (memory.grow (i32.const 1)))
        (if (i32.eq (local.get $old) (i32.const -1)) (then (return (i32.const -1))))
        (local.set $ptr (i32.add (i32.mul (local.get $old) (i32.const 65536)) (i32.const 128)))
        (i32.store (local.get $ptr) (local.get $r))
        (i32.atomic.store (i32.const 0) (local.get $ptr))
        (loop $wait
          (if (i32.atomic.load (i32.const 24)) (then (return (i32.const -2))))
          (br_if $wait (i32.ne (i32.atomic.load (i32.const 4)) (local.get $r))))
        (br $round)))
    (local.get $r))

  ;; Worker B: one long activation. Per round: spin on the mailbox (atomic load only, no
  ;; allocation, no memory.grow), optional refresh ($mode 0 none, 1 memory.grow(0),
  ;; 2 memory.size), then on the pointer: memory.copy ($ops&1), memory.fill ($ops&2),
  ;; i32.atomic.rmw.add ($ops&4); then ack. $warm=1 runs the same code on [1024] without
  ;; the mailbox, so V8 tiers the function up before the real call.
  (func (export "handoff_loop") (param $rounds i32) (param $mode i32) (param $ops i32) (param $warm i32) (result i32)
    (local $r i32) (local $ptr i32) (local $last i32) (local $sink i32)
    (if (i32.eqz (local.get $warm)) (then (i32.atomic.store (i32.const 16) (i32.const 1))))
    (block $out
      (loop $round
        (br_if $out (i32.ge_u (local.get $r) (local.get $rounds)))
        (local.set $r (i32.add (local.get $r) (i32.const 1)))
        (if (local.get $warm)
          (then (local.set $ptr (i32.const 1024)))
          (else
            (loop $spin
              (local.set $ptr (i32.atomic.load (i32.const 0)))
              (br_if $spin (i32.eq (local.get $ptr) (local.get $last))))
            (local.set $last (local.get $ptr))
            (i32.atomic.store (i32.const 12) (local.get $r))))
        (if (i32.eq (local.get $mode) (i32.const 1)) (then (i32.atomic.store (i32.const 36) (memory.grow (i32.const 0)))))
        (if (i32.eq (local.get $mode) (i32.const 2)) (then (i32.atomic.store (i32.const 36) (memory.size))))
        (if (i32.and (local.get $ops) (i32.const 1)) (then
          (i32.atomic.store (i32.const 8) (i32.const 1))
          (memory.copy (local.get $ptr) (i32.const 64) (i32.const 16))))
        (if (i32.and (local.get $ops) (i32.const 2)) (then
          (i32.atomic.store (i32.const 8) (i32.const 2))
          (memory.fill (local.get $ptr) (i32.const 0x5a) (i32.const 16))))
        (if (i32.and (local.get $ops) (i32.const 4)) (then
          (i32.atomic.store (i32.const 8) (i32.const 3))
          (local.set $sink (i32.add (local.get $sink) (i32.atomic.rmw.add (local.get $ptr) (i32.const 1))))))
        (i32.atomic.store (i32.const 8) (i32.const 0))
        (if (i32.eqz (local.get $warm)) (then (i32.atomic.store (i32.const 4) (local.get $r))))
        (br $round)))
    ;; keep $sink live so memory.size / rmw results are not dropped as dead code
    (i32.store (i32.const 32) (local.get $sink))
    (local.get $r))
)
