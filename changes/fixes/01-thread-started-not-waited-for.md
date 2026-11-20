- **A thread started and not waited for says which test catches it.** A missed `thread::spawn(..)`
  or `thread::Builder`'s `spawn` said only that no test checked what the statement does. Its card
  now says no test waits for the thread and checks what it did, and to join its handle or wait on a
  channel its closure sends to first. A scoped thread's `spawn` keeps the card any statement gets,
  as its scope waits for it. The Java edition's cards name the test the same way for threads,
  pools, interrupts, locks and daemon flags.
