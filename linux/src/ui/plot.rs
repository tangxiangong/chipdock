use super::*;
impl Workbench {
    pub(super) fn plot_page(&self, cx: &mut Context<Self>) -> Div {
        use gpui_kit::component::chart::LineChart;
        let mut page = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.button("detach-plot","在独立窗口打开",cx,|s,_,cx|{if let Some(id)=s.active&&let Err(e)=s.open_panel(super::auxiliary::Panel::Plot(id),cx){s.report_error(e.to_string(),cx);}}))
            .child(self.editor("plot", cx))
            .child(self.row(&["plot_sample","plot_field"]))
            .child(self.button("plot-preview","试算样例",cx,|s,_,cx|{let p=s.snapshot(cx);match serde_json::from_str::<Value>(p.get("plot")){Ok(config)=>{let hex=config["mode"]=="binary";if let Some(id)=s.active{s.submit(Command::Workbench(id,json!({"action":"plot","config":config,"sample":p.get("plot_sample"),"hex":hex})),cx)}},Err(e)=>s.report_error(e.to_string(),cx)}}))
            .child(self.button("plot-clear","清空曲线",cx,|s,_,cx|{s.plot_revision+=1;s.plot_points.clear();s.plot_dropped=0;cx.notify();}))
            .child(format!("超出保留范围：{} 条",self.plot_dropped))
            .child(
                self.button("plot-toggle", "开始／暂停曲线", cx, |s, _, cx| {
                    s.plotting = !s.plotting;
                    cx.notify();
                }),
            );
        let mut series: BTreeMap<String, Vec<(String, f64)>> = BTreeMap::new();
        let field = self.fields["plot_field"].read(cx).value();
        for point in &self.plot_points {
            if let Some(values) = point["values"].as_object() {
                for (name, value) in values {
                    if !field.is_empty() && field.as_ref() != name {
                        continue;
                    }
                    if let Some(value) = value.as_f64() {
                        series
                            .entry(name.clone())
                            .or_default()
                            .push((point["seq"].to_string(), value));
                    }
                }
            }
        }
        page = page.child(
            self.button("export-plot", "导出曲线 JSON…", cx, |s, _, cx| {
                match serde_json::to_vec_pretty(&s.plot_points) {
                    Ok(bytes) => s.export_bytes("ChipDock-plot.json", bytes, cx),
                    Err(e) => s.report_error(e.to_string(), cx),
                }
            }),
        );
        page = page.child(
            self.button("export-plot-csv", "导出曲线 CSV…", cx, |s, _, cx| {
                let bytes = chipdock_gpui::presentation::plot_csv(
                    &s.plot_points,
                    s.fields["plot_field"].read(cx).value().as_ref(),
                );
                s.export_bytes("ChipDock-Plot.csv", bytes, cx);
            }),
        );
        for (index, (name, data)) in series.into_iter().enumerate() {
            page = page.child(
                div().child(name).h(px(200.)).child(
                    LineChart::new(data)
                        .id(("plot", index))
                        .x(|v: &(String, f64)| v.0.clone())
                        .y(|v: &(String, f64)| v.1),
                ),
            );
        }
        page
    }
}
