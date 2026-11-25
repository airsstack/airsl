airsstack.stdio.write(tostring(select(2, pcall(airsstack.time.parse, "2024-07-01 12:00 America/New_York", "%Y-%m-%d %H:%M %Q"))):gsub("\n.*", "") .. "\n")
