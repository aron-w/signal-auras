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
  leader = "F9",
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
        key "Enter",
      },
    },
    {
      requires_held = { "<Leader>" },
      trigger = { "<LClick>", "<LClick>" },
      mode = "passthrough",
      loop = {
        while_held = { "<LClick>" },
        before = macro {
          key_down "Ctrl",
        },
        repeat = {
          every_ms = 65,
          macro = macro {
            mouse_click "left",
          },
        },
        after = macro {
          key_up "Ctrl",
        },
      },
    },
  },
  presses = {
    {
      requires_held = { "<Leader>" },
      trigger = "<WheelUp>",
      mode = "passthrough",
      macro = macro {
        key "Left",
      },
    },
    {
      requires_held = { "<Leader>" },
      trigger = "<WheelDown>",
      mode = "passthrough",
      macro = macro {
        key "Right",
      },
    },
    {
      requires_held = { "<Leader>" },
      trigger = "<RClick>",
      mode = "consume",
      macro = macro {
        key_down "Alt",
        -- delay(10),
        mouse_click "left",
        -- delay(10),
        key_up "Alt",
      },
    },
    {
      requires_held = { "<Leader>" },
      trigger = "<LClick>",
      mode = "consume",
      macro = macro {
        key_down "Ctrl",
        -- delay(10),
        mouse_click "left",
        -- delay(10),
        key_up "Ctrl",
      },
    },
  },
}
