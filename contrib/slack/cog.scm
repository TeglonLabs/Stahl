(define package-name 'slack/websocket)
(define version "0.1.0")

;; Core library, requires no dependencies.
(define dependencies
  '((#:name stahl-websockets #:path "../libs/stahl-websockets")
    (#:name stahl-webrequests #:path "../libs/stahl-webrequests")))
