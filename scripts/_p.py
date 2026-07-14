path = 'crates/rover-client/src/widgets/sidebar.rs'
with open(path) as f:
    content = f.read()

lines = content.split('\n')
new_lines = []
skip_until_row_content = False
in_fn = False

for i, line in enumerate(lines):
    if 'fn device_row' in line:
        in_fn = True
        new_lines.append(line)
        continue

    if in_fn and line.strip().startswith('let label = column!'):
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
        skip_until_row_content = True
        continue

    if skip_until_row_content:
        if 'let row_content' in line:
            skip_until_row_content = False
            new_lines.append('    let delete_btn = button(text("\u{2715}").size(10).color(colors::DANGER))')
            new_lines.append('        .style(button::text)')
            new_lines.append('        .on_press(Message::DeleteDevice(i));')
            new_lines.append('')
            new_lines.append('    let row_content = row![dot, label, Space::with_width(Length::Fill), edit_icon, delete_btn]')
            continue
        else:
            continue

    new_lines.append(line)

with open(path, 'w') as f:
    f.write('\n'.join(new_lines))
print('Done')
