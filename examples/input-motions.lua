return {
  leader = "F13",
  defaults = {
    inter_action_delay_ms = 0,
  },
  motions = {
    {
      trigger = { "<Leader>", "f", "f" },
      mode = "consume",
      macro = macro {
        text "/search",
      },
    },
    {
      requires_held = { "<Leader>" },
      trigger = { "<LClick>", "<LClick>" },
      mode = "passthrough",
      loop = {
        while_held = { "<LClick>" },
        repeat = {
          every_ms = 65,
          macro = macro {
            mouse_click "left",
          },
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
  },
}
