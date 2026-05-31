return {
  -- scope = { processes = { "steam_app_2694490" } },
  input_provider = {
    backend = "evdev",
    mode = "grab",
    output = "uinput",
    devices = {
      "/dev/input/by-signal-auras/keychron-k5-pro",
      "/dev/input/by-signal-auras/logitech-mouse",
      "/dev/input/by-signal-auras/logitech-mouse-keyboard",
    },
  },
  leader = "Ctrl",
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
      trigger = { "<Leader>", "<WheelUp>" },
      mode = "passthrough",
      macro = macro {
        key "Left",
      },
    },
    {
      trigger = { "<Leader>", "<WheelDown>" },
      mode = "passthrough",
      macro = macro {
        key "Right",
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
