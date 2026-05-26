return {
  bindings = {
    {
      trigger = {
        modifiers = { "Ctrl" },
        mouse = { wheel = "up" },
      },
      macro = macro {
        key "Left",
      },
    },
    {
      trigger = {
        modifiers = { "Ctrl" },
        mouse = { wheel = "down" },
      },
      macro = macro {
        key "Right",
      },
    },
    {
      trigger = {
        modifiers = { "Ctrl" },
        mouse = { button = "left" },
      },
      macro = macro {
        key "Alt+Right",
        text "hello world",
        key "Enter",
      },
    },
  },
}
