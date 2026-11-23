- **A test that fails with no mutant in, or caught mutants and fails on its own, exits 3, as in
  the Java edition.** It exited 2, as a usage error does; a script that checks for 2 there now
  sees 3. There's a way past it, leaving the test out with a reason, so it's told apart from a
  mistake in the command. Every other failure still exits 2 until 2.0.
