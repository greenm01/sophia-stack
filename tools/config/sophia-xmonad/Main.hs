{-# LANGUAGE FlexibleContexts #-}
{-# OPTIONS_GHC -Wno-missing-signatures #-}

module Main (main) where

import qualified Data.Map.Strict as Map
import Graphics.X11.Types
import XMonad
import XMonad.Layout.Spiral (spiral)
import XMonad.Layout.ThreeColumns (ThreeCol (ThreeColMid))
import qualified XMonad.StackSet as StackSet

-- The compatibility server synthesizes only these bounded policy actions.
-- Session services retain launching, closing applications, and logout.
sophiaKeys config =
  Map.fromList
    [ ((modMask config, xK_j), windows StackSet.focusDown),
      ((modMask config, xK_k), windows StackSet.focusUp),
      ((modMask config, xK_space), sendMessage NextLayout),
      ((modMask config .|. shiftMask, xK_space), setLayout $ layoutHook config)
    ]

sophiaLayouts =
  ThreeColMid 1 (3 / 100) (1 / 2)
    ||| Tall 1 (3 / 100) (1 / 2)
    ||| Mirror (Tall 1 (3 / 100) (1 / 2))
    ||| Full
    ||| spiral (6 / 7)

main :: IO ()
main =
  xmonad $
    def
      { borderWidth = 0,
        focusedBorderColor = "#000000",
        keys = sophiaKeys,
        layoutHook = sophiaLayouts,
        manageHook = mempty,
        modMask = mod1Mask,
        normalBorderColor = "#000000",
        terminal = "/bin/false",
        workspaces = map show [1 .. 9 :: Int]
      }
