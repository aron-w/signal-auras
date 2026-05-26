return {
  scope = { processes = { "steam_app_2694490" } },
  input_provider = {
    backend = "evdev",
    mode = "observe",
    output = "uinput",
    devices = "all",
  },
  leader = "F3",
  defaults = {
    inter_action_delay_ms = 200,
  },
  motions = {
    {
      trigger = { "F5" },
      mode = "passthrough",
      macro = macro {
        key "Enter",
        text "/hideout",
        delay(50),
        key "Enter",
      },
    },
    {
      trigger = { "<Leader>", "<LClick>", "<LClick>" },
      mode = "passthrough",
      repeat = {
        while_held = { "<LClick>" },
        interval_ms = { min = 50, max = 80 },
        macro = macro {
          mouse_click "left",
        },
      },
    },
  },
}
