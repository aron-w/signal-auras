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
      trigger = { "<Leader>", "<LClick>", "<LClick>" },
      mode = "passthrough",
      repeat = {
        while_held = { "<Leader>", "<LClick>" },
        interval_ms = { min = 50, max = 80 },
        macro = macro {
          mouse_click "left",
        },
      },
    },
  },
}
