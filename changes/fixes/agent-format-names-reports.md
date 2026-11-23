- **The agent format names the report files a run wrote.** It never said there were any, so an
  agent took it there was no report and ran again with `--report=html`. A `reports:` line after the
  verdict names them as the summary's Reports row does,
  `reports: target/mutastic/mutation.json · target/mutastic/mutation.html`, and with `--report=none`
  there's no such line.
