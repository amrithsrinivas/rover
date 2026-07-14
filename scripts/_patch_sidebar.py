path = 'crates/rover-client/src/widgets/sidebar.rs'
with open(path) as f:
    content = f.read()

# Find the old label block - between "fn device_row" and "let row_content"
lines = content.split('\n')
new_lines = []
in_device_row = False
skip_until_row_content = False
i = 0
while i < len(lines):
    line = lines[i]
    if 'fn device_row' in line:
        in_device_row = True
        new_lines.append(line)
        i += 1
        continue

    if in_device_row and line.strip().startswith('let label = column!'):
        # Found label - skip until after the delete_btn
        new_lines.append('    let edit_icon = button(text("\u{270E}").size(10).color(colors::TEXT_MUTED))')
        new_lines.append('        .style(button::text)')
        new_lines.append('        .on_press(Message::StartRename(i));')
        new_lines.append('')
        new_lines.append('    let label: Element<Message> = if app.editing_device == Some(i) {')
        new_lines.append('        text_input("name", &app.rename_value)')
        new_lines.append('            .on_input(Message::SetRenameValue)')
        new_lines.append('            .on_submit(Message::ConfirmRename(i))')
        new_lines.append('            .size(13)')
        new_lines.append('            .padding(4)')
        new_lines.append('            .into()')
        new_lines.append('    } else {')
        new_lines.append('        column![')
        new_lines.append('            text(&d.profile.name).size(13).color(colors::TEXT),')
        new_lines.append('            text(&d.profile.address).size(10).color(colors::TEXT_MUTED),')
        new_lines.append('        ]')
        new_lines.append('        .spacing(2)')
        new_lines.append('        .into()')
        new_lines.append('    };')
        new_lines.append('')
        
        # skip old lines until row_content
        skip_until_row_content = True
        i += 1
        continue
    
    if skip_until_row_content:
        if 'let row_content' in line:
            # Update row_content to include edit_icon
            skip_until_row_content = False
            new_lines.append('    let delete_btn = button(text("\u{2715}").size(10).color(colors::DANGER))')
            new_lines.append('        .style(button::text)')
            new_lines.append('        .on_press(Message::DeleteDevice(i));')
            new_lines.append('')
            new_lines.append('    let row_content = row![dot, label, Space::with_width(Length::Fill), edit_icon, delete_btn]')
            i += 1
            continue
        else:
            i += 1
            continue

    new_lines.append(line)
    i += 1

with open(path, 'w') as f:
    f.write('\n'.join(new_lines))
print('Done')
