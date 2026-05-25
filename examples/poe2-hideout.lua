return {
  scope = { processes = { "poe2.exe" } },
  hotkeys = {
    ["F5"] = macro {
      key "Enter",
      text "/hideout",
      delay 50,
      key "Enter",
    },
  },
}
