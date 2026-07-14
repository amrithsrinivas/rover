path = 'crates/rover-client/src/widgets/sidebar.rs'
with open(path) as f:
    content = f.read()

old = '''    let label = column![
        text(&d.profile.name).size(13).color(colors::TEXT),
        text(&d.profile.address).size(10).color(colors::TEXT_MUTED),
    ]
    .spacing(2);

    // Delete button (only show on hover or for disconnected devices)
    let delete_btn = button(text("" + chr(0x2715) + "").size(10).color(colors::DANGER))
        .style(button::text)
        .on_press(Message::DeleteDevice(i));

    let row_content = row![dot, label, Space::with_width(Length::Fill), delete_btn]'''

# Find the actual occurrence
import re
escaped = content.replace('text("\\u{2715}")', 'text("X")')  # just to check
if old not in content:
    # Try with the actual Unicode char
    old_real = '''    let label = column![
        text(&d.profile.name).size(13).color(colors::TEXT),
        text(&d.profile.address).size(10).color(colors::TEXT_MUTED),
    ]
    .spacing(2);

    // Delete button (only show on hover or for disconnected devices)
    let delete_btn = button(text("'''
    
    # Split approach - find the key lines
    idx_label = content.find('    let label = column![')
    idx_row = content.find('    let row_content = row![dot, label, Space::with_width(Length::Fill), delete_btn]')
    print(f'label at {idx_label}, row at {idx_row}')
    
    import sys
    sys.exit(1)
